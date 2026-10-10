//! Photon's motion system: how shell elements animate in and out.
//!
//! An [`Entrance`] describes how an element appears: any mix of fading,
//! sliding in from an edge, sharpening from a blur and growing to its size,
//! with a duration, delay and [`Curve`]. Start from a preset such as
//! [`Entrance::rise`] and adjust it, then apply it with
//! [`AnimateIn::animate_in`]:
//!
//! ```ignore
//! panel.animate_in("dialog-panel", Entrance::rise().delay(Speed::Quick))
//! ```
//!
//! The same entrance played backwards and a little faster is the element's
//! exit. A removed element is gone at once, so keep it with [`Presence`] while
//! it leaves, and play the exit with [`Animate::animate`]:
//!
//! ```ignore
//! if let Some((menu, transition)) = self.menu_presence.sync(self.open_menu, Entrance::popover(), cx) {
//!     surface.animate("menu", Entrance::popover(), transition)
//! }
//! ```
//!
//! Every entrance and exit respects the system's reduced-motion setting: the
//! element appears in its final state, and disappears, at once.
//!
//! GPUI has no transforms, so a slide offsets the element from its laid-out
//! position (without moving its siblings), and growing changes its size; use
//! growing only on fixed-size elements such as icons. Animate the element
//! itself, and position it with a parent.

// A toolkit: not every preset, curve and edge is in use yet.
#![allow(dead_code)]

use gpui::{
    Animation, AnimationElement, AnimationExt, Context, ElementId, IntoElement, Motion,
    SpringConfig, Styled, Task, ease_in_out, ease_out_quint, linear, px,
};
use std::time::{Duration, Instant};

/// How far an element moves as it enters, in pixels.
pub(super) mod distance {
    /// A hint of movement, for popovers and dropdowns.
    pub(in super::super) const HINT: f32 = 4.0;
    /// A small nudge, for bars and menus.
    pub(in super::super) const NUDGE: f32 = 8.0;
    /// A short shift, for panels and dialogs.
    pub(in super::super) const SHIFT: f32 = 12.0;
    /// Travel across part of a panel, for sidebars and sheets.
    pub(in super::super) const TRAVEL: f32 = 32.0;
}

/// The blur radius an element sharpens from as it comes into focus, in pixels.
const FOCUS_BLUR: f32 = 6.0;

/// Standard durations, so related motion moves at related speeds.
#[derive(Clone, Copy, Debug)]
pub(super) enum Speed {
    /// Small feedback: chips, icons, hover reveals. 120 ms.
    Quick,
    /// Most entrances: bars, menus, panels. 200 ms.
    Standard,
    /// Large or prominent surfaces. 320 ms.
    Gentle,
}

impl Speed {
    pub(super) const fn duration(self) -> Duration {
        Duration::from_millis(match self {
            Self::Quick => 120,
            Self::Standard => 200,
            Self::Gentle => 320,
        })
    }
}

impl From<Speed> for Duration {
    fn from(speed: Speed) -> Self {
        speed.duration()
    }
}

/// How motion progresses over its duration.
#[derive(Clone, Copy, Debug)]
pub(super) enum Curve {
    /// Constant speed.
    Linear,
    /// Starts fast and settles gently. The default for entrances.
    EaseOut,
    /// Starts and ends slowly.
    EaseInOut,
    /// Overshoots its target a little and settles back, for a playful pop.
    Overshoot,
    /// A physical spring sampled over the duration.
    Spring(SpringConfig),
}

impl Curve {
    /// A firm spring that settles quickly with almost no bounce.
    pub(super) const SNAPPY: Self = Self::Spring(SpringConfig::new(420.0, 32.0, 1.0));
    /// A softer spring with a visible bounce.
    pub(super) const BOUNCY: Self = Self::Spring(SpringConfig::new(300.0, 14.0, 1.0));

    fn motion(self, duration: Duration) -> Motion {
        let motion = Motion::new(duration);
        match self {
            Self::Linear => motion.with_easing(linear),
            Self::EaseOut => motion.with_easing(ease_out_quint()),
            Self::EaseInOut => motion.with_easing(ease_in_out),
            Self::Overshoot => motion.with_easing(ease_out_back),
            Self::Spring(config) => motion.with_spring(config),
        }
    }
}

/// The edge an element slides in from.
#[derive(Clone, Copy, Debug)]
pub(super) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// How an element appears. Build one from a preset or [`Entrance::new`],
/// then apply it with [`AnimateIn::animate_in`].
#[derive(Clone, Copy, Debug)]
pub(super) struct Entrance {
    duration: Duration,
    delay: Duration,
    curve: Curve,
    /// The opacity the element starts at.
    opacity_from: Option<f32>,
    /// The offset the element starts at, in pixels.
    offset_from: Option<(f32, f32)>,
    /// The blur radius the element starts at, in pixels.
    blur_from: Option<f32>,
    /// The element's final size and the fraction of it the element starts at.
    grow: Option<(f32, f32)>,
}

impl Entrance {
    /// An entrance with no effects yet, at standard speed, easing out.
    pub(super) const fn new() -> Self {
        Self {
            duration: Speed::Standard.duration(),
            delay: Duration::ZERO,
            curve: Curve::EaseOut,
            opacity_from: None,
            offset_from: None,
            blur_from: None,
            grow: None,
        }
    }

    // Presets.

    /// Fades in quickly. For chips and small notices.
    pub(super) const fn fade() -> Self {
        Self::new().fade_from(0.0).speed(Speed::Quick)
    }

    /// Fades in while rising into place. For panels and dialogs.
    pub(super) const fn rise() -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(Edge::Bottom, distance::SHIFT)
    }

    /// Fades in while falling into place. For bars and menus that open
    /// below what opened them.
    pub(super) const fn fall() -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(Edge::Top, distance::NUDGE)
    }

    /// Fades in quickly while dropping slightly from what opened it. For
    /// popovers, dropdowns and context menus.
    pub(super) const fn popover() -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(Edge::Top, distance::HINT)
            .speed(Speed::Quick)
    }

    /// Fades in while sliding in from `edge` across a panel's width. For
    /// sidebars and sheets.
    pub(super) const fn slide_in(edge: Edge) -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(edge, distance::TRAVEL)
            .speed(Speed::Gentle)
    }

    /// Fades in while coming into focus from a blur. For content that
    /// replaces other content in place.
    pub(super) const fn focus() -> Self {
        Self::new().fade_from(0.0).blur_from(FOCUS_BLUR)
    }

    /// Grows from small to `size` with a little overshoot. For icons.
    pub(super) const fn pop(size: f32) -> Self {
        Self::new()
            .grow_from(size, 0.35)
            .curve(Curve::Overshoot)
            .speed(Speed::Standard)
    }

    // Effects.

    /// Starts at `opacity` and fades to fully opaque.
    pub(super) const fn fade_from(mut self, opacity: f32) -> Self {
        self.opacity_from = Some(opacity);
        self
    }

    /// Starts `distance` pixels toward `edge` and slides into place.
    pub(super) const fn slide_from(mut self, edge: Edge, distance: f32) -> Self {
        self.offset_from = Some(match edge {
            Edge::Top => (0.0, -distance),
            Edge::Bottom => (0.0, distance),
            Edge::Left => (-distance, 0.0),
            Edge::Right => (distance, 0.0),
        });
        self
    }

    /// Slides up into place from `distance` pixels below.
    pub(super) const fn slide_up(self, distance: f32) -> Self {
        self.slide_from(Edge::Bottom, distance)
    }

    /// Slides down into place from `distance` pixels above.
    pub(super) const fn slide_down(self, distance: f32) -> Self {
        self.slide_from(Edge::Top, distance)
    }

    /// Slides left into place from `distance` pixels to the right.
    pub(super) const fn slide_left(self, distance: f32) -> Self {
        self.slide_from(Edge::Right, distance)
    }

    /// Slides right into place from `distance` pixels to the left.
    pub(super) const fn slide_right(self, distance: f32) -> Self {
        self.slide_from(Edge::Left, distance)
    }

    /// Starts blurred by `radius` pixels and sharpens.
    pub(super) const fn blur_from(mut self, radius: f32) -> Self {
        self.blur_from = Some(radius);
        self
    }

    /// Grows a square element to `size`, starting at `fraction` of it.
    pub(super) const fn grow_from(mut self, size: f32, fraction: f32) -> Self {
        self.grow = Some((size, fraction));
        self
    }

    // Timing.

    pub(super) const fn speed(mut self, speed: Speed) -> Self {
        self.duration = speed.duration();
        self
    }

    pub(super) const fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Waits before starting, to stagger related entrances.
    pub(super) fn delay(mut self, delay: impl Into<Duration>) -> Self {
        self.delay = delay.into();
        self
    }

    pub(super) const fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    /// How long the entrance takes, including its delay.
    pub(super) fn total_duration(&self) -> Duration {
        self.delay + self.duration
    }

    /// How long the exit takes: quicker than the entrance, with no delay.
    pub(super) fn exit_duration(&self) -> Duration {
        self.duration * 3 / 4
    }

    fn animation(&self) -> Animation {
        Animation::new(self.curve.motion(self.duration).with_delay(self.delay))
    }

    /// The exit accelerates away rather than settling, whatever the entrance's curve.
    fn exit_animation(&self) -> Animation {
        Animation::new(Motion::new(self.exit_duration()).with_easing(ease_in_cubic))
    }

    /// Styles `element` at `progress` through the entrance, from 0 to 1.
    /// Overshooting curves can pass 1.
    fn apply<E: Styled>(&self, mut element: E, progress: f32) -> E {
        let remaining = 1.0 - progress;
        if let Some(from) = self.opacity_from {
            element = element.opacity((from + (1.0 - from) * progress).clamp(0.0, 1.0));
        }
        if let Some((x, y)) = self.offset_from {
            element = element.left(px(x * remaining)).top(px(y * remaining));
        }
        if let Some(radius) = self.blur_from {
            let radius = radius * remaining;
            if radius > 0.0 {
                element = element.blur(px(radius));
            }
        }
        if let Some((size, fraction)) = self.grow {
            element = element.size(px(size * (fraction + (1.0 - fraction) * progress)));
        }
        element
    }
}

impl Default for Entrance {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether an element is arriving or leaving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Transition {
    Enter,
    Exit,
}

impl Transition {
    const fn name(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Exit => "exit",
        }
    }
}

/// Animates any styled element in or out with an [`Entrance`].
pub(super) trait AnimateIn: Styled + IntoElement + Sized + 'static {
    /// Plays `entrance` the first time the element is shown under `id`.
    /// A new `id` plays it again.
    fn animate_in(self, id: impl Into<ElementId>, entrance: Entrance) -> AnimationElement<Self> {
        self.with_animation(id, entrance.animation(), move |element, progress| {
            entrance.apply(element, progress)
        })
    }

    /// Plays `entrance` backwards, so the element leaves the way it came.
    /// Use an `id` distinct from the entrance's, so the exit starts afresh.
    fn animate_out(self, id: impl Into<ElementId>, entrance: Entrance) -> AnimationElement<Self> {
        self.with_animation(id, entrance.exit_animation(), move |element, progress| {
            entrance.apply(element, 1.0 - progress)
        })
    }

    /// Animates in or out by `transition`, under an id for each.
    fn animate(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        transition: Transition,
    ) -> AnimationElement<Self> {
        let id = ElementId::from((id.into(), transition.name()));
        match transition {
            Transition::Enter => self.animate_in(id, entrance),
            Transition::Exit => self.animate_out(id, entrance),
        }
    }
}

impl<E: Styled + IntoElement + 'static> AnimateIn for E {}

/// Keeps a value on screen while it animates out after its owner drops it.
///
/// Each render, pass the owner's current value to [`Presence::sync`] and draw
/// what it returns: the current value entering, or the last one leaving until
/// its exit finishes.
pub(super) struct Presence<T> {
    shown: Option<T>,
    leaving: Option<(T, Instant)>,
    /// Redraws the owner once the exit ends, so the value disappears.
    _expiry: Option<Task<()>>,
}

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self {
            shown: None,
            leaving: None,
            _expiry: None,
        }
    }
}

impl<T: Clone + 'static> Presence<T> {
    /// What to draw now, and whether it is entering or leaving `entrance`'s way.
    pub(super) fn sync<V: 'static>(
        &mut self,
        current: Option<T>,
        entrance: Entrance,
        cx: &mut Context<V>,
    ) -> Option<(T, Transition)> {
        if let Some(current) = current {
            self.shown = Some(current.clone());
            self.leaving = None;
            self._expiry = None;
            return Some((current, Transition::Enter));
        }
        let exit = entrance.exit_duration();
        if let Some(last) = self.shown.take()
            && !cx.reduce_motion()
        {
            self.leaving = Some((last, Instant::now()));
            self._expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(exit).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            }));
        }
        match &self.leaving {
            Some((last, since)) if since.elapsed() < exit => Some((last.clone(), Transition::Exit)),
            _ => {
                self.leaving = None;
                None
            }
        }
    }
}

/// A value moving smoothly between 0 and 1, for layout that animates as a
/// whole, such as a sidebar opening and the page narrowing beside it, where
/// an entrance on one element cannot help. Its owner reads [`Tween::value`]
/// as it draws, and asks for another frame while [`Tween::is_running`].
#[derive(Clone, Copy, Debug)]
pub(super) struct Tween {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
}

impl Tween {
    /// A tween resting at `value`.
    pub(super) fn at(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            started: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    /// Moves to `target` over `speed`, from wherever it is now, so turning
    /// back midway never jumps. Reduced motion moves at once.
    pub(super) fn animate_to(&mut self, target: f32, speed: Speed, cx: &gpui::App) {
        if self.to == target {
            return;
        }
        self.from = self.value();
        self.to = target;
        self.started = Instant::now();
        self.duration = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            speed.duration()
        };
    }

    /// Rests at `value` at once.
    pub(super) fn jump_to(&mut self, value: f32) {
        *self = Self::at(value);
    }

    /// Where it is now, eased in and out.
    pub(super) fn value(&self) -> f32 {
        let elapsed = self.started.elapsed();
        if elapsed >= self.duration {
            return self.to;
        }
        let progress = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        mix(self.from, self.to, ease_in_out_cubic(progress))
    }

    pub(super) fn target(&self) -> f32 {
        self.to
    }

    pub(super) fn is_running(&self) -> bool {
        self.started.elapsed() < self.duration
    }
}

/// The value `progress` of the way from `from` to `to`.
pub(super) fn mix(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

/// Starts and ends gently, for motion that may reverse.
fn ease_in_out_cubic(delta: f32) -> f32 {
    if delta < 0.5 {
        4.0 * delta * delta * delta
    } else {
        1.0 - (-2.0 * delta + 2.0).powi(3) / 2.0
    }
}

/// Starts slowly and accelerates away.
fn ease_in_cubic(delta: f32) -> f32 {
    delta * delta * delta
}

/// Eases out past the target and settles back.
fn ease_out_back(delta: f32) -> f32 {
    const OVERSHOOT: f32 = 1.70158;
    let t = delta - 1.0;
    1.0 + (OVERSHOOT + 1.0) * t * t * t + OVERSHOOT * t * t
}
