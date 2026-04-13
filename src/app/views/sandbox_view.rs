//! Sandbox terminal view — raw PS1 mode.
//!
//! Sandbox tabs render as a plain terminal grid (no smart input, no blocks).
//! All keyboard input is forwarded directly to the sandbox bridge.

use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::keyboard::{Key, NamedKey};

impl super::super::App {
    /// Handle keyboard input when the active tab is a sandbox.
    pub(crate) fn handle_sandbox_keyboard(&mut self, event: &WindowEvent) {
        let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    logical_key,
                    text,
                    state: ElementState::Pressed,
                    ..
                },
            ..
        } = event
        else {
            return;
        };

        let ctrl = self.modifiers.control_key();
        let super_key = self.modifiers.super_key();

        if super_key {
            return;
        }

        let bytes: Option<Vec<u8>> = if ctrl {
            match logical_key.as_ref() {
                Key::Character(c) => {
                    let ch = c.chars().next().unwrap_or('\0');
                    if ch.is_ascii_lowercase() || ch.is_ascii_uppercase() {
                        Some(vec![(ch.to_ascii_lowercase() as u8) - b'a' + 1])
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            match logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => Some(b"\r".to_vec()),
                Key::Named(NamedKey::Backspace) => Some(b"\x7f".to_vec()),
                Key::Named(NamedKey::Tab) => Some(b"\t".to_vec()),
                Key::Named(NamedKey::Escape) => Some(b"\x1b".to_vec()),
                Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
                Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
                Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
                Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
                Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
                Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
                Key::Named(NamedKey::PageUp) => Some(b"\x1b[5~".to_vec()),
                Key::Named(NamedKey::PageDown) => Some(b"\x1b[6~".to_vec()),
                Key::Named(NamedKey::Delete) => Some(b"\x1b[3~".to_vec()),
                _ => text.as_ref().and_then(|txt| {
                    let s = txt.to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.into_bytes())
                    }
                }),
            }
        };

        if let Some(data) = bytes {
            self.send_input_to_active(&data);
        }
        self.request_redraw();
    }
}
