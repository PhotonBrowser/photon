//! Translates GPUI input events into Photon Engine page events.

use super::super::engine::EngineSession;
use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseExitEvent,
    MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, TouchPhase,
};

/// An editing command the page runs from its shortcut, such as ⌘C.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl EditCommand {
    /// The key that runs the command with the platform's shortcut modifier,
    /// and whether Shift is held too.
    fn shortcut(self) -> (&'static str, bool) {
        match self {
            Self::Undo => ("z", false),
            Self::Redo => ("z", true),
            Self::Cut => ("x", false),
            Self::Copy => ("c", false),
            Self::Paste => ("v", false),
            Self::SelectAll => ("a", false),
        }
    }

    /// The command `keystroke` runs, if it is one's shortcut.
    fn for_keystroke(keystroke: &Keystroke) -> Option<Self> {
        [
            Self::Undo,
            Self::Redo,
            Self::Cut,
            Self::Copy,
            Self::Paste,
            Self::SelectAll,
        ]
        .into_iter()
        .find(|command| {
            let (key, shift) = command.shortcut();
            keystroke.key == key && keystroke.modifiers == shortcut_modifiers(shift)
        })
    }
}

/// The platform's shortcut modifier, with Shift when asked.
fn shortcut_modifiers(shift: bool) -> Modifiers {
    Modifiers {
        shift,
        ..Modifiers::secondary_key()
    }
}

#[derive(Default)]
pub(super) struct WebViewInput {
    viewport_origin: (f32, f32),
    pointer_inside: bool,
    history_swipe_gesture_id: u64,
    next_history_swipe_gesture_id: u64,
    history_swipe_touch_ended: bool,
}

impl WebViewInput {
    /// Where the page sits in its window.
    pub(super) fn viewport_origin(&self) -> (f32, f32) {
        self.viewport_origin
    }

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
            false,
            false,
            0,
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
            false,
            false,
            0,
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
            false,
            false,
            0,
        );
    }

    pub(super) fn mouse_exit(&mut self, session: &mut EngineSession, event: &MouseExitEvent) {
        self.mouse_leave(session, event.modifiers);
    }

    pub(super) fn scroll(
        &mut self,
        session: &mut EngineSession,
        event: &ScrollWheelEvent,
        can_go_back: bool,
        can_go_forward: bool,
        history_swipe_gesture_id: u64,
        history_swipe_captured: bool,
    ) {
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
        let physical_delta_x = f64::from(physical_horizontal_delta(event));
        let is_native_trackpad_gesture = precise
            && event.has_native_phase
            && event.is_swipe_tracking_enabled
            && !event.is_momentum
            && !history_swipe_captured
            && !matches!(event.touch_phase, TouchPhase::Ended | TouchPhase::Cancelled)
            && physical_delta_x.abs() > delta.1.abs();
        let history_swipe_back_enabled =
            is_native_trackpad_gesture && can_go_back && physical_delta_x > 0.0;
        let history_swipe_forward_enabled =
            is_native_trackpad_gesture && can_go_forward && physical_delta_x < 0.0;

        if precise && delta.0.abs() > 0.0 {
            super::super::trace(format_args!(
                "trackpad-scroll id={history_swipe_gesture_id} phase={:?} delta=({:.2},{:.2}) inverted={} swipe_enabled={} momentum={} history=({}, {}) captured={} eligible=({}, {})",
                event.touch_phase,
                delta.0,
                delta.1,
                event.is_direction_inverted_from_device,
                event.is_swipe_tracking_enabled,
                event.is_momentum,
                can_go_back,
                can_go_forward,
                history_swipe_captured,
                history_swipe_back_enabled,
                history_swipe_forward_enabled,
            ));
        }

        // Normalize GPUI's scroll direction to the Engine's wheel-delta convention,
        // matching the pixel-delta conversion in the existing Qt shell.
        let delta = if history_swipe_captured {
            (0.0, 0.0)
        } else {
            (-delta.0, -delta.1)
        };
        let phase = match (
            event.is_momentum,
            event.is_momentum_ended,
            event.touch_phase,
        ) {
            (true, false, _) => 2,
            (_, true, _) | (_, _, TouchPhase::Ended | TouchPhase::Cancelled) => 3,
            _ => 1,
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
            history_swipe_back_enabled,
            history_swipe_forward_enabled,
            history_swipe_gesture_id,
        );
    }

    pub(super) fn history_swipe_gesture_id(&mut self, event: &ScrollWheelEvent) -> u64 {
        // A gesture starts when none is under way, or when touch begins again
        // after it ended.
        let starts_gesture = !event.is_momentum
            && (self.history_swipe_gesture_id == 0
                || (event.touch_phase == TouchPhase::Started && self.history_swipe_touch_ended));
        if starts_gesture {
            self.next_history_swipe_gesture_id =
                self.next_history_swipe_gesture_id.wrapping_add(1).max(1);
            self.history_swipe_gesture_id = self.next_history_swipe_gesture_id;
        }

        let gesture_id = self.history_swipe_gesture_id;
        if event.touch_phase == TouchPhase::Ended {
            self.history_swipe_touch_ended = true;
        }
        if event.is_momentum_ended || event.touch_phase == TouchPhase::Cancelled {
            self.history_swipe_gesture_id = 0;
            self.history_swipe_touch_ended = false;
        } else if event.touch_phase == TouchPhase::Started && !event.is_momentum {
            self.history_swipe_touch_ended = false;
        }
        gesture_id
    }

    pub(super) fn end_history_swipe(&mut self, session: &mut EngineSession, gesture_id: u64) {
        if gesture_id == 0 {
            return;
        }
        session.send_pointer(
            4, 0.0, 0.0, 0, 0, false, false, false, false, 0.0, 0.0, true, 3, 0, false, false,
            gesture_id,
        );
        if self.history_swipe_gesture_id == gesture_id {
            self.history_swipe_gesture_id = 0;
            self.history_swipe_touch_ended = false;
        }
    }

    pub(super) fn key_down(&mut self, session: &mut EngineSession, event: &KeyDownEvent) -> bool {
        let (key, code_point) =
            keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
        let modifiers = event.keystroke.modifiers;
        let insert_text = code_point != 0 && !modifiers.control && !modifiers.platform;
        // The page runs editing shortcuts itself; consuming them keeps the
        // Edit menu's matching item from running them a second time.
        let should_consume = insert_text
            || event.keystroke.key_char.is_none()
            || EditCommand::for_keystroke(&event.keystroke).is_some();
        session.send_key(key, true, code_point, modifiers, event.is_held, insert_text);
        should_consume
    }

    /// Runs `command` in the page by pressing its shortcut, as the keyboard
    /// would, since the page handles editing from those keys.
    pub(super) fn edit(&mut self, session: &mut EngineSession, command: EditCommand) {
        let (key, shift) = command.shortcut();
        let (key_code, code_point) = keyboard_key(key, Some(key));
        let modifiers = shortcut_modifiers(shift);
        session.send_key(key_code, true, code_point, modifiers, false, false);
        session.send_key(key_code, false, code_point, modifiers, false, false);
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
            false,
            false,
            0,
        );
        self.pointer_inside = false;
    }
}

/// Horizontal scroll distance in the direction the user's fingers moved.
pub(super) fn physical_horizontal_delta(event: &ScrollWheelEvent) -> f32 {
    let delta = match event.delta {
        ScrollDelta::Pixels(delta) => f32::from(delta.x),
        ScrollDelta::Lines(_) => 0.0,
    };
    if event.is_direction_inverted_from_device {
        -delta
    } else {
        delta
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn may_begin_and_began_share_one_gesture_id() {
        let mut input = WebViewInput::default();
        let event = ScrollWheelEvent {
            touch_phase: TouchPhase::Started,
            has_native_phase: true,
            ..Default::default()
        };

        let may_begin_id = input.history_swipe_gesture_id(&event);
        let began_id = input.history_swipe_gesture_id(&event);

        assert_ne!(may_begin_id, 0);
        assert_eq!(may_begin_id, began_id);
    }

    #[test]
    fn a_new_gesture_after_touch_end_gets_a_fresh_id() {
        let mut input = WebViewInput::default();
        let mut event = ScrollWheelEvent {
            touch_phase: TouchPhase::Started,
            has_native_phase: true,
            ..Default::default()
        };
        let first_id = input.history_swipe_gesture_id(&event);

        event.touch_phase = TouchPhase::Ended;
        assert_eq!(input.history_swipe_gesture_id(&event), first_id);

        event.touch_phase = TouchPhase::Started;
        assert_ne!(input.history_swipe_gesture_id(&event), first_id);
    }

    #[test]
    fn cancellation_clears_the_interrupted_gesture_id() {
        let mut input = WebViewInput::default();
        let mut event = ScrollWheelEvent {
            touch_phase: TouchPhase::Started,
            has_native_phase: true,
            ..Default::default()
        };
        let interrupted_id = input.history_swipe_gesture_id(&event);

        event.touch_phase = TouchPhase::Cancelled;
        assert_eq!(input.history_swipe_gesture_id(&event), interrupted_id);

        event.touch_phase = TouchPhase::Moved;
        assert_ne!(input.history_swipe_gesture_id(&event), interrupted_id);
    }

    #[test]
    fn horizontal_scroll_direction_matches_physical_motion() {
        let event = |x, inverted| ScrollWheelEvent {
            delta: ScrollDelta::Pixels(gpui::point(gpui::px(x), gpui::px(0.0))),
            is_direction_inverted_from_device: inverted,
            ..Default::default()
        };

        assert_eq!(physical_horizontal_delta(&event(12.0, false)), 12.0);
        assert_eq!(physical_horizontal_delta(&event(-12.0, true)), 12.0);
        assert_eq!(physical_horizontal_delta(&event(-12.0, false)), -12.0);
        assert_eq!(physical_horizontal_delta(&event(12.0, true)), -12.0);
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    #[test]
    fn recognises_only_exact_editing_shortcuts() {
        let keystroke = |text| Keystroke::parse(text).unwrap();
        assert_eq!(
            EditCommand::for_keystroke(&keystroke("secondary-c")),
            Some(EditCommand::Copy)
        );
        assert_eq!(
            EditCommand::for_keystroke(&keystroke("secondary-shift-z")),
            Some(EditCommand::Redo)
        );
        assert_eq!(
            EditCommand::for_keystroke(&keystroke("secondary-alt-c")),
            None
        );
        assert_eq!(EditCommand::for_keystroke(&keystroke("c")), None);
        assert_eq!(EditCommand::for_keystroke(&keystroke("secondary-t")), None);
    }
}
