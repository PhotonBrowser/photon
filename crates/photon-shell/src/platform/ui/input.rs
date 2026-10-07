//! Translates GPUI input events into Photon Engine page events.

use super::super::engine::EngineSession;
use gpui::{
    KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, ScrollDelta, ScrollWheelEvent, TouchPhase,
};

#[derive(Default)]
pub(super) struct WebViewInput {
    viewport_origin: (f32, f32),
    pointer_inside: bool,
}

impl WebViewInput {
    pub(super) fn set_viewport_origin(&mut self, x: f32, y: f32) {
        self.viewport_origin = (x, y);
    }

    pub(super) fn mouse_down(&mut self, session: &mut EngineSession, event: &MouseDownEvent) {
        let (button, buttons) = mouse_button(event.button);
        let (x, y) = self.content_position(event.position);
        self.pointer_inside = true;
        session.send_pointer(
            2,
            x,
            y,
            button,
            buttons,
            event.modifiers.shift,
            event.modifiers.control,
            event.modifiers.alt,
            event.modifiers.platform,
            0.0,
            0.0,
            false,
            0,
            event.click_count as i32,
        );
    }

    pub(super) fn mouse_up(&mut self, session: &mut EngineSession, event: &MouseUpEvent) {
        let (button, _) = mouse_button(event.button);
        let (x, y) = self.content_position(event.position);
        session.send_pointer(
            3,
            x,
            y,
            button,
            0,
            event.modifiers.shift,
            event.modifiers.control,
            event.modifiers.alt,
            event.modifiers.platform,
            0.0,
            0.0,
            false,
            0,
            event.click_count as i32,
        );
    }

    pub(super) fn mouse_move(
        &mut self,
        session: &mut EngineSession,
        event: &MouseMoveEvent,
        inside_viewport: bool,
    ) {
        if !inside_viewport {
            self.mouse_leave(session, event.modifiers);
            return;
        }

        let (x, y) = self.content_position(event.position);
        self.pointer_inside = true;
        session.send_pointer(
            0,
            x,
            y,
            0,
            event
                .pressed_button
                .map(mouse_button)
                .map_or(0, |(_, buttons)| buttons),
            event.modifiers.shift,
            event.modifiers.control,
            event.modifiers.alt,
            event.modifiers.platform,
            0.0,
            0.0,
            false,
            0,
            0,
        );
    }

    pub(super) fn mouse_exit(&mut self, session: &mut EngineSession, event: &MouseExitEvent) {
        self.mouse_leave(session, event.modifiers);
    }

    pub(super) fn scroll(&mut self, session: &mut EngineSession, event: &ScrollWheelEvent) {
        let (x, y) = self.content_position(event.position);
        let (delta, precise) = match event.delta {
            ScrollDelta::Pixels(delta) => (
                (f64::from(f32::from(delta.x)), f64::from(f32::from(delta.y))),
                true,
            ),
            ScrollDelta::Lines(delta) => (
                (f64::from(delta.x) * 40.0, f64::from(delta.y) * 40.0),
                false,
            ),
        };
        // Normalize AppKit/GPUI's sign to the Engine's wheel-delta convention,
        // matching the pixel-delta conversion in the existing Qt shell.
        let delta = (-delta.0, -delta.1);
        let phase = match event.touch_phase {
            TouchPhase::Started | TouchPhase::Moved => 1,
            TouchPhase::Ended | TouchPhase::Cancelled => 3,
        };
        session.send_pointer(
            4,
            x,
            y,
            0,
            0,
            event.modifiers.shift,
            event.modifiers.control,
            event.modifiers.alt,
            event.modifiers.platform,
            delta.0,
            delta.1,
            precise,
            phase,
            0,
        );
    }

    pub(super) fn key_down(&mut self, session: &mut EngineSession, event: &KeyDownEvent) -> bool {
        let (key, code_point) =
            keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
        let modifiers = event.keystroke.modifiers;
        let insert_text = code_point != 0 && !modifiers.control && !modifiers.platform;
        let should_consume = insert_text || event.keystroke.key_char.is_none();
        session.send_key(key, true, code_point, modifiers, event.is_held, insert_text);
        should_consume
    }

    pub(super) fn key_up(&mut self, session: &mut EngineSession, event: &KeyUpEvent) {
        let (key, code_point) =
            keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
        session.send_key(
            key,
            false,
            code_point,
            event.keystroke.modifiers,
            false,
            false,
        );
    }

    fn content_position(&self, position: gpui::Point<gpui::Pixels>) -> (f64, f64) {
        (
            f64::from((f32::from(position.x) - self.viewport_origin.0).max(0.0)),
            f64::from((f32::from(position.y) - self.viewport_origin.1).max(0.0)),
        )
    }

    fn mouse_leave(&mut self, session: &mut EngineSession, modifiers: gpui::Modifiers) {
        if !self.pointer_inside {
            return;
        }
        session.send_pointer(
            1,
            0.0,
            0.0,
            0,
            0,
            modifiers.shift,
            modifiers.control,
            modifiers.alt,
            modifiers.platform,
            0.0,
            0.0,
            false,
            0,
            0,
        );
        self.pointer_inside = false;
    }
}

fn mouse_button(button: MouseButton) -> (i32, u8) {
    match button {
        MouseButton::Left => (1, 1),
        MouseButton::Right => (2, 2),
        MouseButton::Middle => (4, 4),
        MouseButton::Navigate(gpui::NavigationDirection::Back) => (8, 8),
        MouseButton::Navigate(gpui::NavigationDirection::Forward) => (16, 16),
    }
}

fn keyboard_key(key: &str, key_char: Option<&str>) -> (u16, u32) {
    let code_point = key_char
        .and_then(|text| text.chars().next())
        .map(u32::from)
        .unwrap_or(0);
    let key_code = match key.to_ascii_lowercase().as_str() {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" | "return" => 0x0d,
        "escape" => 0x1b,
        "space" => 0x20,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "end" => 0x23,
        "home" => 0x24,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "delete" => 0x2e,
        key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() => {
            u16::from(key.as_bytes()[0].to_ascii_uppercase())
        }
        key if key.strip_prefix('f').is_some_and(|number| {
            number
                .parse::<u8>()
                .is_ok_and(|number| (1..=12).contains(&number))
        }) =>
        {
            0x70 + key[1..].parse::<u16>().unwrap_or(1) - 1
        }
        _ => 0,
    };
    (key_code, code_point)
}
