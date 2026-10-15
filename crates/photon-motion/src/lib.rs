//! Reusable motion primitives for GPUI applications.
//!
//! An [`Entrance`] describes how an element appears: any mix of fading,
//! sliding in from an edge, sharpening from a blur and growing to its size,
//! with a duration, delay and [`Curve`]. Start from a preset such as
//! [`Entrance::rise`] and adjust it, then apply it with
//! [`AnimateIn::animate_in`]:
//!
//! ```ignore
//! panel.animate_in("dialog-panel", Entrance::rise(), &MotionSettings::default())
//! ```
//!
//! The same entrance played backwards and a little faster is the element's
//! exit. A removed element is gone at once, so keep it with [`Presence`] while
//! it leaves, and play the exit with [`Animate::animate`]:
//!
//! ```ignore
//! let settings = MotionSettings::default();
//! if let Some((menu, transition)) = self.menu_presence.sync(self.open_menu, Entrance::popover(), &settings, cx) {
//!     surface.animate("menu", Entrance::popover(), transition, &settings)
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

use gpui::{
    Animation, AnimationElement, AnimationExt, App, AsyncApp, Context, ElementId, IntoElement,
    Motion, SpringAnimation, SpringConfig, StatefulInteractiveElement, Styled, Task, ease_in_out,
    ease_out_quint, linear, px,
};
use std::time::{Duration, Instant};

/// Application-level motion preferences used by the reusable animation APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionSettings {
    /// Whether animations are enabled.
    pub enabled: bool,
    /// Whether system accessibility settings request reduced motion.
    pub reduced_motion: bool,
    /// Duration for small feedback and icon motion.
    pub quick_duration_ms: u16,
    /// Duration for common panels and menus.
    pub standard_duration_ms: u16,
    /// Duration for larger, prominent surfaces.
    pub gentle_duration_ms: u16,
    /// Relative strength of entrance effects, in percent.
    pub effect_strength_percent: u16,
}

impl Default for MotionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            reduced_motion: false,
            quick_duration_ms: 120,
            standard_duration_ms: 200,
            gentle_duration_ms: 320,
            effect_strength_percent: 100,
        }
    }
}

/// Supplies the preferences used by GPUI motion primitives.
///
/// Implement this trait to connect the framework to an application's own
/// persisted animation settings.
pub trait MotionConfig {
    /// Returns the framework settings represented by this configuration.
    fn motion_settings(&self) -> MotionSettings;
}

impl MotionConfig for MotionSettings {
    fn motion_settings(&self) -> MotionSettings {
        *self
    }
}

/// How far an element moves as it enters, in pixels.
pub mod distance {
    /// A hint of movement, for popovers and dropdowns.
    pub const HINT: f32 = 4.0;
    /// A small nudge, for bars and menus.
    pub const NUDGE: f32 = 8.0;
    /// A short shift, for panels and dialogs.
    pub const SHIFT: f32 = 12.0;
    /// Travel across part of a panel, for sidebars and sheets.
    pub const TRAVEL: f32 = 32.0;
}

const PRESS_SCALE: f32 = 0.97;
const PRESS_SPRING: SpringConfig = SpringConfig::new(480.0, 44.0, 1.0);
pub const CONTROL_THUMB_HELD_STRETCH_X: f32 = 1.2;
/// Maximum track extension as a fraction of thumb width, leaving room for the
/// held thumb extension inside the slider's half-thumb end inset.
pub const SLIDER_EDGE_STRETCH_MAX: f32 = 0.3;
pub const SLIDER_EDGE_STRETCH_SPEED_THRESHOLD: f32 = 700.0;
pub const SLIDER_EDGE_STRETCH_SPEED_RANGE: f32 = 1_500.0;

/// The blur radius an element sharpens from as it comes into focus, in pixels.
const FOCUS_BLUR: f32 = 6.0;

/// Standard durations, so related motion moves at related speeds.
#[derive(Clone, Copy, Debug)]
pub enum Speed {
    /// Small feedback: chips, icons, hover reveals. Defaults to 120 ms.
    Quick,
    /// Most entrances: bars, menus, panels. Defaults to 200 ms.
    Standard,
    /// Large or prominent surfaces. Defaults to 320 ms.
    Gentle,
}

impl Speed {
    fn duration(self, settings: &MotionSettings) -> Duration {
        let millis = match self {
            Self::Quick => settings.quick_duration_ms,
            Self::Standard => settings.standard_duration_ms,
            Self::Gentle => settings.gentle_duration_ms,
        }
        .min(2_000);
        Duration::from_millis(if settings.enabled && !settings.reduced_motion {
            u64::from(millis)
        } else {
            0
        })
    }
}

/// How motion progresses over its duration.
#[derive(Clone, Copy, Debug)]
pub enum Curve {
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
    pub const SNAPPY: Self = Self::Spring(SpringConfig::new(420.0, 32.0, 1.0));
    /// A softer spring with a visible bounce.
    pub const BOUNCY: Self = Self::Spring(BOUNCY_SPRING);

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

pub const BOUNCY_SPRING: SpringConfig = SpringConfig::new(300.0, 14.0, 1.0);

/// The edge an element slides in from.
#[derive(Clone, Copy, Debug)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// How an element appears. Build one from a preset or [`Entrance::new`],
/// then apply it with [`AnimateIn::animate_in`].
#[derive(Clone, Copy, Debug)]
pub struct Entrance {
    speed: Speed,
    duration_override: Option<Duration>,
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
    /// The uniform scale the element starts at.
    scale_from: Option<f32>,
}

impl Entrance {
    /// An entrance with no effects yet, at standard speed, easing out.
    pub const fn new() -> Self {
        Self {
            speed: Speed::Standard,
            duration_override: None,
            delay: Duration::ZERO,
            curve: Curve::EaseOut,
            opacity_from: None,
            offset_from: None,
            blur_from: None,
            grow: None,
            scale_from: None,
        }
    }

    // Presets.

    /// Fades in quickly. For chips and small notices.
    pub const fn fade() -> Self {
        Self::new().fade_from(0.0).speed(Speed::Quick)
    }

    /// Fades in while rising into place. For panels and dialogs.
    pub const fn rise() -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(Edge::Bottom, distance::SHIFT)
    }

    /// Fades in while falling into place. For bars and menus that open
    /// below what opened them.
    pub const fn fall() -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(Edge::Top, distance::NUDGE)
    }

    /// Fades in quickly while dropping slightly from what opened it. For
    /// popovers, dropdowns and context menus.
    pub const fn popover() -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(Edge::Top, distance::HINT)
            .speed(Speed::Quick)
    }

    /// Fades in while sliding in from `edge` across a panel's width. For
    /// sidebars and sheets.
    pub const fn slide_in(edge: Edge) -> Self {
        Self::new()
            .fade_from(0.0)
            .slide_from(edge, distance::TRAVEL)
            .speed(Speed::Gentle)
    }

    /// Fades in while coming into focus from a blur. For content that
    /// replaces other content in place.
    pub const fn focus() -> Self {
        Self::new().fade_from(0.0).blur_from(FOCUS_BLUR)
    }

    /// Grows from small to `size` with a little overshoot. For icons.
    pub const fn pop(size: f32) -> Self {
        Self::new()
            .grow_from(size, 0.35)
            .curve(Curve::Overshoot)
            .speed(Speed::Standard)
    }

    /// Scales into place from a smaller size. For controls and compact surfaces.
    pub const fn scale_from(mut self, scale: f32) -> Self {
        self.scale_from = Some(scale);
        self
    }

    // Effects.

    /// Starts at `opacity` and fades to fully opaque.
    pub const fn fade_from(mut self, opacity: f32) -> Self {
        self.opacity_from = Some(opacity);
        self
    }

    /// Starts `distance` pixels toward `edge` and slides into place.
    pub const fn slide_from(mut self, edge: Edge, distance: f32) -> Self {
        self.offset_from = Some(match edge {
            Edge::Top => (0.0, -distance),
            Edge::Bottom => (0.0, distance),
            Edge::Left => (-distance, 0.0),
            Edge::Right => (distance, 0.0),
        });
        self
    }

    /// Slides up into place from `distance` pixels below.
    pub const fn slide_up(self, distance: f32) -> Self {
        self.slide_from(Edge::Bottom, distance)
    }

    /// Slides down into place from `distance` pixels above.
    pub const fn slide_down(self, distance: f32) -> Self {
        self.slide_from(Edge::Top, distance)
    }

    /// Slides left into place from `distance` pixels to the right.
    pub const fn slide_left(self, distance: f32) -> Self {
        self.slide_from(Edge::Right, distance)
    }

    /// Slides right into place from `distance` pixels to the left.
    pub const fn slide_right(self, distance: f32) -> Self {
        self.slide_from(Edge::Left, distance)
    }

    /// Starts blurred by `radius` pixels and sharpens.
    pub const fn blur_from(mut self, radius: f32) -> Self {
        self.blur_from = Some(radius);
        self
    }

    /// Grows a square element to `size`, starting at `fraction` of it.
    pub const fn grow_from(mut self, size: f32, fraction: f32) -> Self {
        self.grow = Some((size, fraction));
        self
    }

    // Timing.

    pub const fn speed(mut self, speed: Speed) -> Self {
        self.speed = speed;
        self.duration_override = None;
        self
    }

    pub const fn duration(mut self, duration: Duration) -> Self {
        self.duration_override = Some(duration);
        self
    }

    /// Waits before starting, to stagger related entrances.
    pub fn delay(mut self, delay: impl Into<Duration>) -> Self {
        self.delay = delay.into();
        self
    }

    pub const fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    /// How long the entrance takes, including its delay.
    fn resolved_duration(&self, settings: &MotionSettings) -> Duration {
        if !settings.enabled || settings.reduced_motion {
            return Duration::ZERO;
        }
        self.duration_override
            .unwrap_or_else(|| self.speed.duration(settings))
    }

    /// How long the exit takes: quicker than the entrance, with no delay.
    pub fn exit_duration(&self, config: &impl MotionConfig) -> Duration {
        self.resolved_duration(&config.motion_settings()) * 3 / 4
    }

    /// How long the entrance takes, including its delay.
    pub fn total_duration(&self, config: &impl MotionConfig) -> Duration {
        let settings = config.motion_settings();
        if settings.enabled && !settings.reduced_motion {
            self.delay + self.resolved_duration(&settings)
        } else {
            Duration::ZERO
        }
    }

    fn animation(&self, settings: &MotionSettings) -> Animation {
        let duration = self.resolved_duration(settings);
        let delay = if settings.enabled && !settings.reduced_motion {
            self.delay
        } else {
            Duration::ZERO
        };
        Animation::new(self.curve.motion(duration).with_delay(delay))
    }

    /// The exit accelerates away rather than settling, whatever the entrance's curve.
    fn exit_animation(&self, settings: &MotionSettings) -> Animation {
        Animation::new(Motion::new(self.exit_duration(settings)).with_easing(ease_in_cubic))
    }

    /// Styles `element` at `progress` through the entrance, from 0 to 1.
    /// Overshooting curves can pass 1.
    fn apply<E: Styled>(&self, element: E, progress: f32, settings: &MotionSettings) -> E {
        self.apply_with_offset(element, progress, settings, (0.0, 0.0))
    }

    /// Styles `element` while preserving its existing relative position.
    fn apply_with_offset<E: Styled>(
        &self,
        mut element: E,
        progress: f32,
        settings: &MotionSettings,
        base_offset: (f32, f32),
    ) -> E {
        let effect_scale = f32::from(settings.effect_strength_percent.min(300)) / 100.0;
        let remaining = 1.0 - progress;
        if let Some(from) = self.opacity_from {
            element = element.opacity((from + (1.0 - from) * progress).clamp(0.0, 1.0));
        }
        if let Some((x, y)) = self.offset_from {
            element = element
                .left(px(base_offset.0 + x * effect_scale * remaining))
                .top(px(base_offset.1 + y * effect_scale * remaining));
        }
        if let Some(radius) = self.blur_from {
            let radius = radius * effect_scale * remaining;
            if radius > 0.0 {
                element = element.blur(px(radius));
            }
        }
        if let Some((size, fraction)) = self.grow {
            let start = (1.0 - (1.0 - fraction) * effect_scale).clamp(0.2, 1.0);
            element = element.size(px(size * (start + (1.0 - start) * progress)));
        }
        if let Some(from) = self.scale_from {
            let start = (1.0 - (1.0 - from) * effect_scale).clamp(0.2, 1.0);
            let scale = start + (1.0 - start) * progress;
            element = element.scale(scale);
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
pub enum Transition {
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
pub trait AnimateIn: Styled + IntoElement + Sized + 'static {
    /// Plays `entrance` the first time the element is shown under `id`.
    /// A new `id` plays it again.
    fn animate_in(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &impl MotionConfig,
    ) -> AnimationElement<Self> {
        let settings = config.motion_settings();
        self.with_animation(
            id,
            entrance.animation(&settings),
            move |element, progress| entrance.apply(element, progress, &settings),
        )
    }

    /// Plays an entrance while preserving an element's existing relative
    /// offset, such as the temporary displacement used during tab reordering.
    fn animate_in_with_offset(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &impl MotionConfig,
        base_offset: (f32, f32),
    ) -> AnimationElement<Self> {
        let settings = config.motion_settings();
        self.with_animation(
            id,
            entrance.animation(&settings),
            move |element, progress| {
                entrance.apply_with_offset(element, progress, &settings, base_offset)
            },
        )
    }

    /// Plays `entrance` backwards, so the element leaves the way it came.
    /// Use an `id` distinct from the entrance's, so the exit starts afresh.
    fn animate_out(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &impl MotionConfig,
    ) -> AnimationElement<Self> {
        let settings = config.motion_settings();
        self.with_animation(
            id,
            entrance.exit_animation(&settings),
            move |element, progress| entrance.apply(element, 1.0 - progress, &settings),
        )
    }

    /// Animates in or out by `transition`, under an id for each.
    fn animate(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        transition: Transition,
        config: &impl MotionConfig,
    ) -> AnimationElement<Self> {
        let id = ElementId::from((id.into(), transition.name()));
        match transition {
            Transition::Enter => self.animate_in(id, entrance, config),
            Transition::Exit => self.animate_out(id, entrance, config),
        }
    }
}

impl<E: Styled + IntoElement + 'static> AnimateIn for E {}

/// Animates a scalar value with a GPUI spring while letting the caller style
/// the element from each sampled value.
pub fn animate_spring<E>(
    element: E,
    id: impl Into<ElementId>,
    spring: SpringConfig,
    epsilon: f32,
    from: f32,
    to: f32,
    update: impl Fn(E, f32) -> E + 'static,
) -> impl IntoElement
where
    E: Styled + IntoElement + 'static,
{
    element.with_spring(
        id,
        SpringAnimation::new(spring)
            .with_epsilon(epsilon)
            .to(to)
            .from(from),
        update,
    )
}

/// Adds a small, eased scale response while a pointer or touch press is active.
pub fn pressable<E: StatefulInteractiveElement + Styled>(element: E) -> E {
    scale_transition(element).active(|style| style.scale(PRESS_SCALE))
}

/// Adds the shared spring used by controls that scale while pressed.
pub fn scale_transition<E: StatefulInteractiveElement + Styled>(element: E) -> E {
    element.transitions(|transitions| transitions.scale(Motion::spring(PRESS_SPRING)))
}

fn control_stretch_motion(settings: &MotionSettings) -> Motion {
    Curve::EaseOut.motion(Speed::Quick.duration(settings))
}

/// Applies the same bounded easing to track stretch and its directional offset.
pub fn control_scale_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    tracking: bool,
    config: &impl MotionConfig,
) -> E {
    let settings = config.motion_settings();
    let motion = if tracking {
        Motion::new(Duration::ZERO)
    } else {
        control_stretch_motion(&settings)
    };
    element.transitions(|transitions| transitions.scale(motion.clone()).left(motion))
}

/// Moves a control between states with the same bounded easing used by its thumb.
pub fn control_left_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    config: &impl MotionConfig,
) -> E {
    let settings = config.motion_settings();
    element.transitions(|transitions| transitions.left(control_stretch_motion(&settings)))
}

/// Animates a reusable thumb's width and offset with the same bounded easing.
pub fn control_thumb_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    config: &impl MotionConfig,
) -> E {
    let settings = config.motion_settings();
    let motion = control_stretch_motion(&settings);
    element.transitions(|transitions| transitions.w(motion.clone()).left(motion))
}

/// Keeps a value on screen while it animates out after its owner drops it.
///
/// Each render, pass the owner's current value to [`Presence::sync`] and draw
/// what it returns: the current value entering, or the last one leaving until
/// its exit finishes.
pub struct Presence<T> {
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
    pub fn sync<V: 'static>(
        &mut self,
        current: Option<T>,
        entrance: Entrance,
        config: &impl MotionConfig,
        cx: &mut Context<V>,
    ) -> Option<(T, Transition)> {
        let settings = config.motion_settings();
        if let Some(current) = current {
            self.shown = Some(current.clone());
            self.leaving = None;
            self._expiry = None;
            return Some((current, Transition::Enter));
        }
        let exit = entrance.exit_duration(&settings);
        if let Some(last) = self.shown.take()
            && !settings.reduced_motion
            && !cx.reduce_motion()
            && settings.enabled
        {
            self.leaving = Some((last, Instant::now()));
            self._expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(exit).await;
                update_app(cx, |cx| this.update(cx, |_, cx| cx.notify()).ok()).await;
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
pub struct Tween {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
}

impl Tween {
    /// A tween resting at `value`.
    pub fn at(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            started: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    /// Moves to `target` over `speed`, from wherever it is now, so turning
    /// back midway never jumps. Reduced motion moves at once.
    pub fn animate_to(
        &mut self,
        target: f32,
        speed: Speed,
        config: &impl MotionConfig,
        cx: &gpui::App,
    ) {
        let settings = config.motion_settings();
        if self.to == target {
            return;
        }
        self.from = self.value();
        self.to = target;
        self.started = Instant::now();
        self.duration = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            speed.duration(&settings)
        };
    }

    /// Moves to `target` over an explicit duration, from wherever it is now.
    /// Reduced motion or disabled animations move directly to the target.
    pub fn animate_to_for(
        &mut self,
        target: f32,
        duration: Duration,
        config: &impl MotionConfig,
        cx: &App,
    ) {
        let settings = config.motion_settings();
        if self.to == target {
            return;
        }
        self.from = self.value();
        self.to = target;
        self.started = Instant::now();
        self.duration = if !settings.enabled || settings.reduced_motion || cx.reduce_motion() {
            Duration::ZERO
        } else {
            duration
        };
    }

    /// Rests at `value` at once.
    pub fn jump_to(&mut self, value: f32) {
        *self = Self::at(value);
    }

    /// Where it is now, eased in and out.
    pub fn value(&self) -> f32 {
        let elapsed = self.started.elapsed();
        if elapsed >= self.duration {
            return self.to;
        }
        let progress = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        mix(self.from, self.to, ease_in_out_cubic(progress))
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn is_running(&self) -> bool {
        self.started.elapsed() < self.duration
    }
}

/// The value `progress` of the way from `from` to `to`.
pub fn mix(from: f32, to: f32, progress: f32) -> f32 {
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

/// How long an asynchronous callback waits before retrying a busy GPUI app.
const UPDATE_RETRY_DELAY: Duration = Duration::from_millis(8);
const UPDATE_ATTEMPTS: u32 = 500;

async fn update_app<R>(cx: &AsyncApp, update: impl FnOnce(&mut App) -> R) -> Option<R> {
    let mut update = Some(update);
    for _ in 0..UPDATE_ATTEMPTS {
        if let Some(result) = cx.try_update(|cx| (update.take().expect("runs once"))(cx)) {
            return Some(result);
        }
        cx.background_executor().timer(UPDATE_RETRY_DELAY).await;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Entrance, MotionSettings};
    use std::time::Duration;

    #[test]
    fn presets_use_shared_durations_and_quicker_exits() {
        let settings = MotionSettings::default();
        let entrance = Entrance::fade();

        assert_eq!(
            entrance.total_duration(&settings),
            Duration::from_millis(120)
        );
        assert_eq!(entrance.exit_duration(&settings), Duration::from_millis(90));
    }

    #[test]
    fn disabled_and_reduced_motion_skip_entrance_and_exit() {
        for settings in [
            MotionSettings {
                enabled: false,
                ..MotionSettings::default()
            },
            MotionSettings {
                reduced_motion: true,
                ..MotionSettings::default()
            },
        ] {
            let entrance = Entrance::rise().delay(Duration::from_millis(80));
            assert_eq!(entrance.total_duration(&settings), Duration::ZERO);
            assert_eq!(entrance.exit_duration(&settings), Duration::ZERO);
        }
    }
}
