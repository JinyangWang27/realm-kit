//! Raw terminal key input, translated to menu keys.

use crate::menu::Key;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal,
};
use std::io;

/// Leaves raw mode when dropped, including on `?` errors and panics.
struct RawGuard;

impl RawGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Raw mode is held only while waiting for a key, so rendering keeps plain `\n`.
pub(crate) fn terminal_keys() -> impl Iterator<Item = io::Result<Key>> {
    std::iter::from_fn(|| {
        let _raw = match RawGuard::new() {
            Ok(guard) => guard,
            Err(error) => return Some(Err(error)),
        };
        loop {
            let event = match event::read() {
                Ok(event) => event,
                Err(error) => return Some(Err(error)),
            };
            let Event::Key(key) = event else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            return Some(Ok(match key.code {
                KeyCode::Char('c' | 'd') if ctrl => Key::Quit,
                KeyCode::Up => Key::Up,
                KeyCode::Down => Key::Down,
                KeyCode::Enter => Key::Enter,
                KeyCode::Esc => Key::Esc,
                KeyCode::Backspace => Key::Backspace,
                KeyCode::Char(c) if !ctrl => Key::Char(c),
                _ => continue,
            }));
        }
    })
}
