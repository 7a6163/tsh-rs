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

    /// Process a key event and return data to send to server
    pub async fn handle_key_event(&mut self, key_event: KeyEvent) -> TshResult<Option<Vec<u8>>> {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Ok(None), // Signal to exit

            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.current_line.is_empty() {
                    Ok(None) // EOF on empty line
                } else {
                    Ok(Some(vec![]))
                }
            }

            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Clear screen
                let mut stdout = stdout();
                stdout.execute(Clear(ClearType::All))?;
                stdout.execute(MoveTo(0, 0))?;
                self.display_prompt()?;
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                // Add to history if not empty
                if !self.current_line.is_empty() {
                    self.add_to_history(self.current_line.clone());
                }

                // Send the line with newline
                let mut data = self.current_line.as_bytes().to_vec();
                data.push(b'\n');

                // Clear current line for next input
                self.current_line.clear();
                self.cursor_pos = 0;
                self.history_pos = None;

                // Print newline
                println!();

                Ok(Some(data))
            }

            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                if self.cursor_pos > 0 {
                    self.current_line.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                    self.display_prompt()?;
                }
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Delete,
                ..
            } => {
                if self.cursor_pos < self.current_line.len() {
                    self.current_line.remove(self.cursor_pos);
                    self.display_prompt()?;
                }
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Left,
                ..
            } => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.display_prompt()?;
                }
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Right,
                ..
            } => {
                if self.cursor_pos < self.current_line.len() {
                    self.cursor_pos += 1;
                    self.display_prompt()?;
                }
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                self.navigate_history_up()?;
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                self.navigate_history_down()?;
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.cursor_pos = 0;
                self.display_prompt()?;
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.cursor_pos = self.current_line.len();
                self.display_prompt()?;
                Ok(Some(vec![]))
            }

            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                // Send tab for remote completion
                Ok(Some(vec![b'\t']))
            }

            KeyEvent {
                code: KeyCode::Char(c),
                ..
            } => {
                self.current_line.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                self.display_prompt()?;
                Ok(Some(vec![]))
            }

            _ => Ok(Some(vec![])),
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

    /// Navigate up in history
    fn navigate_history_up(&mut self) -> TshResult<()> {
        if self.history.is_empty() {
            return Ok(());
        }

        match self.history_pos {
            None => {
                // First time pressing up, save current line
                self.history_pos = Some(0);
                if let Some(cmd) = self.history.front() {
                    self.current_line = cmd.clone();
                    self.cursor_pos = self.current_line.len();
                    self.display_prompt()?;
                }
            }
            Some(pos) => {
                if pos + 1 < self.history.len() {
                    self.history_pos = Some(pos + 1);
                    if let Some(cmd) = self.history.get(pos + 1) {
                        self.current_line = cmd.clone();
                        self.cursor_pos = self.current_line.len();
                        self.display_prompt()?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Navigate down in history
    fn navigate_history_down(&mut self) -> TshResult<()> {
        match self.history_pos {
            Some(0) => {
                // Back to current input
                self.history_pos = None;
                self.current_line.clear();
                self.cursor_pos = 0;
                self.display_prompt()?;
            }
            Some(pos) => {
                self.history_pos = Some(pos - 1);
                if let Some(cmd) = self.history.get(pos - 1) {
                    self.current_line = cmd.clone();
                    self.cursor_pos = self.current_line.len();
                    self.display_prompt()?;
                }
            }
            None => {}
        }
        Ok(())
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
