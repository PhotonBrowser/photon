//! Icons that move while hovered. An `AnimatedIcon` is an SVG plus the motion
//! it takes on hover, and `animated_icon` turns it into an element.
//!
//! To add one:
//!
//! 1. Vendor its SVG into `assets/icons/heroicons/` or `assets/icons/lucide/`,
//!    keeping the `@license` header, and load it with `heroicon!` or `lucide!`.
//! 2. Describe its hover motion in the catalogue below, following
//!    `settings_cog`.
//! 3. Swap the static icon at the call site for
//!    `animated_icon("<unique id>", settings_cog(), color, size, window, cx)`.
//!
//! The motion moves the whole icon: rotation, scale and translation, on a
//! spring. Animating single parts, changing opacity and keyframe sequences are
//! not supported.

use gpui::{App, Div, ElementId, Stateful, Window, div, prelude::*, px};
use gpui_motion::{Spring, SvgFrame, SvgMotion, SvgMotionState, SvgTarget, Transition};
use std::time::{Duration, Instant};

use super::icons::{FAVOURITE_STAR_SVG, heroicon, icon, lucide};
use super::motion::{MotionConfig, hover_icon_spring};
use super::settings::Settings;

/// An icon and the motion it takes while hovered. It rests until hovered,
/// moves to the hovered target, and moves back when the pointer leaves.
#[derive(Clone, Copy)]
pub(super) struct AnimatedIcon {
    data: &'static [u8],
    hovered: SvgTarget,
    transition: Transition,
}

impl AnimatedIcon {
    /// An icon that moves on hover with the default spring. Add the motion
    /// with `rotate`, `scale`, `translate` or `spring`.
    pub(super) fn new(data: &'static [u8]) -> Self {
        Self {
            data,
            hovered: SvgTarget::default(),
            transition: Transition::Spring(hover_icon_spring()),
        }
    }

    /// Turns the icon to `degrees` around its centre. Angles are unwrapped, so
    /// `360.0` is a full turn.
    pub(super) fn rotate(mut self, degrees: f64) -> Self {
        self.hovered = self.hovered.rotate_degrees(degrees);
        self
    }

    /// Scales the icon around its centre, so `1.2` makes it a fifth larger.
    pub(super) fn scale(mut self, factor: f32) -> Self {
        self.hovered = self.hovered.scale(factor);
        self
    }

    /// Moves the drawing by logical pixels. Layout and the hover area stay put.
    pub(super) fn translate(mut self, x: f32, y: f32) -> Self {
        self.hovered = self.hovered.translate(x, y);
        self
    }

    /// Replaces the default spring with one of this stiffness and damping.
    pub(super) fn spring(mut self, stiffness: f32, damping: f32) -> Self {
        self.transition = Transition::Spring(
            Spring::new(stiffness, damping, 1.0).expect("icon spring parameters are positive"),
        );
        self
    }
}

/// `animated`, drawn `size` square in `color`, moving while the element is
/// hovered. `id` names that element and the icon's retained motion, so it must
/// be unique in the window. The element is `size` square: call `size_full` on
/// it to make the whole parent the hover area.
pub(super) fn animated_icon(
    id: &'static str,
    animated: AnimatedIcon,
    color: u32,
    size: f32,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let motion = window.use_keyed_state(
        ElementId::Name(format!("{id}-motion").into()),
        cx,
        |_, _| MotionState::default(),
    );
    let config = Settings::motion(cx);
    let frame = motion.update(cx, |motion, _| motion.frame(&animated, &config, window));
    let hover = motion.clone();
    div()
        .id(ElementId::Name(format!("{id}-hover").into()))
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .on_hover(move |hovered, _, cx| {
            hover.update(cx, |motion, cx| {
                if motion.hovered != *hovered {
                    motion.hovered = *hovered;
                    cx.notify();
                }
            });
        })
        .child(icon(animated.data, color, size).with_transformation(frame.transformation()))
}

/// Whether the icon is hovered, and where its motion has got to, kept between
/// frames by its element.
struct MotionState {
    hovered: bool,
    motion: SvgMotionState,
    started: Instant,
}

impl Default for MotionState {
    fn default() -> Self {
        Self {
            hovered: false,
            motion: SvgMotionState::default(),
            started: Instant::now(),
        }
    }
}

impl MotionState {
    /// Samples the motion for this frame, and asks for the next frame while it
    /// is still moving.
    fn frame(
        &mut self,
        animated: &AnimatedIcon,
        config: &MotionConfig,
        window: &mut Window,
    ) -> SvgFrame {
        let frame = self.sample(animated, config.allows_motion(), self.started.elapsed());
        if frame.is_animating() {
            window.request_animation_frame();
        }
        frame
    }

    /// Disabled or reduced motion snaps straight to the destination.
    fn sample(&mut self, animated: &AnimatedIcon, allows_motion: bool, now: Duration) -> SvgFrame {
        let target = if self.hovered {
            animated.hovered
        } else {
            SvgTarget::default()
        };
        let motion = SvgMotion::new(target).transition(animated.transition);
        self.motion.sample(motion, now, !allows_motion)
    }
}

// The catalogue. heroicons-animated animates some icons per part, which this
// framework cannot draw; those entries take the nearest whole-icon motion and
// say so.

/// The settings cog from heroicons-animated: half a turn on hover.
pub(super) fn settings_cog() -> AnimatedIcon {
    AnimatedIcon::new(heroicon!("cog-6-tooth")).rotate(180.0)
}

/// The plus from heroicons-animated: half a turn on its own, quicker spring.
pub(super) fn plus() -> AnimatedIcon {
    AnimatedIcon::new(heroicon!("plus"))
        .rotate(180.0)
        .spring(100.0, 15.0)
}

/// The browser menu's dots. heroicons-animated pulses each dot in turn, which
/// this framework cannot draw, so the whole icon grows instead.
pub(super) fn browser_menu_dots() -> AnimatedIcon {
    AnimatedIcon::new(heroicon!("ellipsis-vertical")).scale(1.2)
}

/// A cross for dismissing. heroicons-animated draws it in stroke by stroke,
/// which this framework cannot draw, so it turns a quarter instead.
pub(super) fn dismiss_x() -> AnimatedIcon {
    AnimatedIcon::new(heroicon!("x-mark")).rotate(90.0)
}

/// A filled star, which marks a favourite, so it keeps its filled shape. It
/// grows on hover, where heroicons-animated squashes and stretches it.
pub(super) fn favourite_star() -> AnimatedIcon {
    AnimatedIcon::new(FAVOURITE_STAR_SVG).scale(1.2)
}

/// The sidebar toggle while the sidebar is shown: its panel slides left, out
/// of the way.
pub(super) fn sidebar_hide() -> AnimatedIcon {
    AnimatedIcon::new(lucide!("panel-left")).translate(-2.0, 0.0)
}

/// The sidebar toggle while the sidebar is hidden: its arrow slides right, out
/// of the panel.
pub(super) fn sidebar_show() -> AnimatedIcon {
    AnimatedIcon::new(lucide!("panel-left-open")).translate(2.0, 0.0)
}

/// The back arrow. heroicons-animated pulses its head by 3 pixels, which this
/// framework cannot draw, so the whole arrow moves 2 pixels back instead.
pub(super) fn back_arrow() -> AnimatedIcon {
    AnimatedIcon::new(lucide!("arrow-left")).translate(-2.0, 0.0)
}

/// The forward arrow. It moves 2 pixels forward, mirroring `back_arrow`.
pub(super) fn forward_arrow() -> AnimatedIcon {
    AnimatedIcon::new(lucide!("arrow-right")).translate(2.0, 0.0)
}

/// The reload arrow, keeping its Lucide glyph. The turn and spring are
/// heroicons-animated's arrow-path, a 50 degree turn on a stiffer spring.
pub(super) fn reload_arrow() -> AnimatedIcon {
    AnimatedIcon::new(lucide!("rotate-cw"))
        .rotate(50.0)
        .spring(250.0, 25.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_icon() -> AnimatedIcon {
        AnimatedIcon::new(b"").rotate(180.0)
    }

    #[test]
    fn rests_until_hovered() {
        let animated = test_icon();
        let mut state = MotionState::default();

        let frame = state.sample(&animated, true, Duration::ZERO);

        assert_eq!(frame.target().rotation_degrees(), 0.0);
        assert!(!frame.is_animating());
    }

    #[test]
    fn moves_to_hovered_target_and_settles() {
        let animated = test_icon();
        let mut state = MotionState::default();
        state.sample(&animated, true, Duration::ZERO);
        state.hovered = true;

        let moving = state.sample(&animated, true, Duration::ZERO);
        let settled = state.sample(&animated, true, Duration::from_secs(10));

        assert!(moving.is_animating());
        assert_eq!(settled.target().rotation_degrees(), 180.0);
        assert!(!settled.is_animating());
    }

    #[test]
    fn reduced_motion_snaps_to_hovered_target() {
        let animated = test_icon();
        let mut state = MotionState::default();
        state.sample(&animated, false, Duration::ZERO);
        state.hovered = true;

        let frame = state.sample(&animated, false, Duration::ZERO);

        assert_eq!(frame.target().rotation_degrees(), 180.0);
        assert!(!frame.is_animating());
    }
}
