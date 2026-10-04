//! The page surface and browser input forwarding.

use super::super::engine::EngineSession;
use super::super::presentation::{PresentedSurface, Release};
use super::super::window_settings::{TITLEBAR_HEIGHT, WEBVIEW_CORNER_RADIUS, WEBVIEW_INSET};
use super::theme::colors;
use gpui::{
    Context, FocusHandle, InteractiveElement, KeyDownEvent, KeyUpEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Render, ScrollDelta, ScrollWheelEvent,
    Subscription, SurfaceSource, TouchPhase, Window, div, prelude::*, px, surface,
};
use std::sync::{Arc, Mutex};

use super::super::trace;

pub(in crate::platform) struct PhotonWebView {
    pub(super) external: Option<PresentedSurface>,
    pub(super) session: EngineSession,
    pub(super) focus_handle: FocusHandle,
    pub(super) last_viewport: Option<(i32, i32, u32)>,
    pub(super) _quit_subscription: Option<Subscription>,
}

impl Drop for PhotonWebView {
    fn drop(&mut self) {
        self.prepare_shutdown();
    }
}

impl PhotonWebView {
    pub(in crate::platform) fn present_latest(&mut self, cx: &mut Context<Self>) {
        self.session.drain_releases();
        let Some(presented) = self.session.presentation.take_surface() else {
            return;
        };
        if let Some(retired) = self.external.take() {
            let pending = std::mem::take(&mut *retired.releases_after_completion.lock().unwrap());
            let mut next_releases = presented.releases_after_completion.lock().unwrap();
            next_releases.push(Release { key: retired.key });
            next_releases.extend(pending);
        }
        self.external = Some(presented);
        cx.notify();
    }

    fn handle_mouse_up(&mut self, event: &MouseUpEvent) {
        let (button, _) = mouse_button(event.button);
        let (x, y) = web_content_position(event.position);
        self.session.send_pointer(
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

    pub(super) fn prepare_shutdown(&mut self) {
        if self.session.shutdown_started {
            return;
        }
        let final_releases = if let Some(presented) = self.external.as_ref() {
            let releases = presented.releases_after_completion.clone();
            let mut pending = releases.lock().unwrap();
            if !pending.iter().any(|release| release.key == presented.key) {
                pending.push(Release { key: presented.key });
            }
            drop(pending);
            releases
        } else {
            Arc::new(Mutex::new(Vec::new()))
        };
        self.session.begin_shutdown(final_releases);
    }

    fn update_viewport(&mut self, width: f32, height: f32, scale: f32) {
        let width = width.round() as i32;
        let height = height.round() as i32;
        let viewport = (width, height, scale.to_bits());
        if self.last_viewport == Some(viewport) || width <= 0 || height <= 0 {
            return;
        }

        self.session.resize(width, height, f64::from(scale));
        trace(format_args!(
            "viewport logical={width}x{height} dpr={scale:.2} physical={}x{}",
            (width as f32 * scale).round() as i32,
            (height as f32 * scale).round() as i32,
        ));
        self.last_viewport = Some(viewport);
        // The initial native window's backing scale can differ from the scale
        // available before it exists (for example, Retina 2x). Navigate only
        // after the measured WebView bounds and scale have been sent to Engine.
        self.session.navigate_startup();
    }

    pub(super) fn new(cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let width = 1200;
        let height = 760;
        Ok(Self {
            external: None,
            session: EngineSession::create(width, height, 1.0)?,
            focus_handle: cx.focus_handle(),
            last_viewport: None,
            _quit_subscription: None,
        })
    }
}

impl Render for PhotonWebView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut webview = div()
            .size_full()
            .rounded(px(WEBVIEW_CORNER_RADIUS))
            .bg(gpui::rgb(colors::PAGE_BACKGROUND))
            .id("photon-webview-viewport")
            .on_prepaint(
                cx.listener(|this, event: &gpui::InteractivityPrepaint, window, _| {
                    // Resize from the laid-out WebView bounds, not the native window
                    // bounds minus assumed titlebar/padding values. During live resize,
                    // those assumptions can differ by a frame and offset the page.
                    this.update_viewport(
                        f32::from(event.bounds.size.width),
                        f32::from(event.bounds.size.height),
                        window.scale_factor(),
                    );
                }),
            )
            .track_focus(&self.focus_handle)
            .on_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.session.set_focus(true);
                let (button, buttons) = mouse_button(event.button);
                let (x, y) = web_content_position(event.position);
                this.session.send_pointer(
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
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Navigate(gpui::NavigationDirection::Back),
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Navigate(gpui::NavigationDirection::Forward),
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, _| {
                let (x, y) = web_content_position(event.position);
                this.session.send_pointer(
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
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, _| {
                let (x, y) = web_content_position(event.position);
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
                this.session.send_pointer(
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
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, _| {
                let (key, code_point) =
                    keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
                this.session.send_key(
                    key,
                    true,
                    code_point,
                    event.keystroke.modifiers,
                    event.is_held,
                    event.prefer_character_input && code_point != 0,
                );
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                let (key, code_point) =
                    keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
                this.session.send_key(
                    key,
                    false,
                    code_point,
                    event.keystroke.modifiers,
                    false,
                    false,
                );
            }));
        if let Some(presented) = self.external.as_ref() {
            webview = webview.child(
                surface(SurfaceSource::ExternalMetal(presented.surface.clone()))
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .rounded(px(WEBVIEW_CORNER_RADIUS)),
            );
        }
        webview
    }
}

fn web_content_position(position: gpui::Point<gpui::Pixels>) -> (f64, f64) {
    (
        f64::from((f32::from(position.x) - WEBVIEW_INSET).max(0.0)),
        f64::from((f32::from(position.y) - TITLEBAR_HEIGHT - WEBVIEW_INSET).max(0.0)),
    )
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
        key if key.strip_prefix('f').is_some_and(|n| {
            n.parse::<u8>()
                .is_ok_and(|number| (1..=12).contains(&number))
        }) =>
        {
            0x70 + key[1..].parse::<u16>().unwrap_or(1) - 1
        }
        _ => 0,
    };
    (key_code, code_point)
}
