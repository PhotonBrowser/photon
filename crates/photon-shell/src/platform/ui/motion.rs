//! Photon presets and accessibility policy over the reusable motion engine.

use gpui::{
    A11ySubtreeBuilder, App, Bounds, Element, ElementId, FocusHandle, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, SpringConfig,
    StatefulInteractiveElement, Styled, Window, rgba,
};
use gpui_motion::{
    Color, CustomEasing, Easing, Frame, Motion, MotionElement, MotionValue, PressScaleElement,
    Spring, StyleMotion, StyleProperty, StyleTarget, Target,
};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

pub(super) use gpui_motion::{Axis as ReorderAxis, ReorderGroup};

/// Moves an element's painted and interactive bounds without changing layout.
pub(super) fn translated<E: IntoElement>(
    element: E,
    offset: Point<Pixels>,
) -> TranslatedElement<E::Element> {
    TranslatedElement {
        element: element.into_element(),
        offset,
    }
}

/// Applies a frame-local offset after layout, including to descendant hitboxes.
pub(super) struct TranslatedElement<E> {
    element: E,
    offset: Point<Pixels>,
}

impl<E: Element> Element for TranslatedElement<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.element.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.element.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.element.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let offset = self.offset;
        window.with_element_offset(offset, |window| {
            self.element.prepaint(
                id,
                inspector_id,
                bounds + offset,
                request_layout,
                window,
                cx,
            )
        })
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(
            id,
            inspector_id,
            bounds + self.offset,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }

    fn a11y_role(&self) -> Option<gpui::accesskit::Role> {
        self.element.a11y_role()
    }

    fn is_a11y_hidden(&self) -> bool {
        self.element.is_a11y_hidden()
    }

    fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
        self.element.write_a11y_info(node);
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        self.element.a11y_synthetic_children(prepaint, builder);
    }
}

impl<E: Element> IntoElement for TranslatedElement<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn custom_ease_out() -> Easing {
    static EASING: OnceLock<CustomEasing> = OnceLock::new();
    Easing::Custom(*EASING.get_or_init(|| {
        CustomEasing::new(|t| {
            let remaining = 1.0 - t;
            (1.0 - remaining.powi(5), 5.0 * remaining.powi(4))
        })
        .expect("valid quintic ease-out endpoints")
    }))
}

fn custom_ease_in_out_cubic() -> Easing {
    static EASING: OnceLock<CustomEasing> = OnceLock::new();
    Easing::Custom(*EASING.get_or_init(|| {
        CustomEasing::new(|t| {
            if t < 0.5 {
                (4.0 * t.powi(3), 12.0 * t.powi(2))
            } else {
                let remaining = -2.0 * t + 2.0;
                (1.0 - remaining.powi(3) / 2.0, 3.0 * remaining.powi(2))
            }
        })
        .expect("valid cubic ease-in-out endpoints")
    }))
}

/// Application timing and accessibility preferences used by Photon presets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MotionConfig {
    pub enabled: bool,
    pub reduce_motion: bool,
    pub effect_strength: f32,
    pub timings: MotionTimings,
}

impl MotionConfig {
    pub const fn allows_motion(self) -> bool {
        self.enabled && !self.reduce_motion
    }

    pub const fn duration(self, speed: Speed) -> Duration {
        if !self.allows_motion() {
            return Duration::ZERO;
        }
        match speed {
            Speed::Quick => self.timings.quick,
            Speed::Standard => self.timings.standard,
            Speed::Gentle => self.timings.gentle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MotionTimings {
    pub quick: Duration,
    pub standard: Duration,
    pub gentle: Duration,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Speed {
    Quick,
    Standard,
    Gentle,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Keep the full preset vocabulary available to Photon UI surfaces.
pub(super) enum Curve {
    Linear,
    EaseOut,
    EaseInOut,
    Overshoot,
    Spring(SpringConfig),
}

#[allow(dead_code)] // Keep named spring constants available for future Photon presets.
impl Curve {
    pub const SNAPPY: Self = Self::Spring(SpringConfig::new(420.0, 32.0, 1.0));
    pub const BOUNCY: Self = Self::Spring(SpringConfig::new(300.0, 14.0, 1.0));

    pub(super) fn transition(self, duration: Duration) -> gpui_motion::Transition {
        if duration.is_zero() {
            return gpui_motion::Transition::Instant;
        }
        match self {
            Self::Linear => gpui_motion::Transition::Tween(duration, Easing::Linear),
            Self::EaseOut => gpui_motion::Transition::Tween(duration, custom_ease_out()),
            Self::EaseInOut => gpui_motion::Transition::Tween(duration, Easing::EaseInOut),
            Self::Overshoot => gpui_motion::Transition::Tween(duration, Easing::BackOut),
            Self::Spring(config) => gpui_motion::Transition::Spring(
                Spring::new(
                    config.stiffness,
                    config.damping.max(f32::MIN_POSITIVE),
                    config.mass,
                )
                .expect("Photon spring parameters are positive"),
            ),
        }
    }
}

/// Converts an opaque RGB theme token into the motion engine's linear-light color.
pub(super) fn motion_rgb_color(token: u32) -> Color {
    Color::rgb(token)
}

/// Converts an RGBA theme token into the motion engine's linear-light color.
pub(super) fn motion_rgba_color(token: u32) -> Color {
    let color = rgba(token);
    Color::rgba(
        f64::from(color.color.red),
        f64::from(color.color.green),
        f64::from(color.color.blue),
        f64::from(color.alpha),
    )
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Photon-specific composition of entrance and exit effects.
#[derive(Clone, Copy, Debug)]
pub(super) struct Entrance {
    speed: Speed,
    duration_override: Option<Duration>,
    delay: Duration,
    curve: Curve,
    opacity_from: Option<f32>,
    offset_from: Option<(f32, f32)>,
    blur_from: Option<f32>,
    grow: Option<(f32, f32)>,
    scale_from: Option<f32>,
}

impl Entrance {
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

    pub const fn scale_from(mut self, scale: f32) -> Self {
        self.scale_from = Some(scale);
        self
    }

    pub const fn fade_from(mut self, opacity: f32) -> Self {
        self.opacity_from = Some(opacity);
        self
    }

    pub const fn slide_from(mut self, edge: Edge, distance: f32) -> Self {
        self.offset_from = Some(match edge {
            Edge::Top => (0.0, -distance),
            Edge::Bottom => (0.0, distance),
            Edge::Left => (-distance, 0.0),
            Edge::Right => (distance, 0.0),
        });
        self
    }

    pub const fn slide_up(self, distance: f32) -> Self {
        self.slide_from(Edge::Bottom, distance)
    }

    #[allow(dead_code)]
    pub const fn slide_down(self, distance: f32) -> Self {
        self.slide_from(Edge::Top, distance)
    }

    #[allow(dead_code)]
    pub const fn slide_left(self, distance: f32) -> Self {
        self.slide_from(Edge::Right, distance)
    }

    #[allow(dead_code)]
    pub const fn slide_right(self, distance: f32) -> Self {
        self.slide_from(Edge::Left, distance)
    }

    pub const fn blur_from(mut self, radius: f32) -> Self {
        self.blur_from = Some(radius);
        self
    }

    pub const fn grow_from(mut self, size: f32, fraction: f32) -> Self {
        self.grow = Some((size, fraction));
        self
    }

    pub const fn speed(mut self, speed: Speed) -> Self {
        self.speed = speed;
        self.duration_override = None;
        self
    }

    #[allow(dead_code)]
    pub const fn duration(mut self, duration: Duration) -> Self {
        self.duration_override = Some(duration);
        self
    }

    #[allow(dead_code)]
    pub fn delay(mut self, delay: impl Into<Duration>) -> Self {
        self.delay = delay.into();
        self
    }

    pub const fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    fn resolved_duration(self, config: &MotionConfig) -> Duration {
        if !config.allows_motion() {
            return Duration::ZERO;
        }
        self.duration_override
            .unwrap_or_else(|| config.duration(self.speed))
    }

    pub(super) fn exit_duration(self, config: &MotionConfig) -> Duration {
        self.resolved_duration(config) * 3 / 4
    }

    pub(super) fn total_duration(self, config: &MotionConfig) -> Duration {
        if config.allows_motion() {
            self.delay + self.resolved_duration(config)
        } else {
            Duration::ZERO
        }
    }

    fn targets(
        self,
        config: &MotionConfig,
        base_offset: (f32, f32),
    ) -> (Target, Target, Option<f32>) {
        let strength = config.effect_strength.clamp(0.0, 3.0);
        let mut shown = Target::default();
        let mut hidden = Target::default();

        if let Some(opacity) = self.opacity_from {
            shown = shown.opacity(1.0);
            hidden = hidden.opacity(opacity);
        }
        if let Some((x, y)) = self.offset_from {
            shown = shown.x(base_offset.0).y(base_offset.1);
            hidden = hidden
                .x(base_offset.0 + x * strength)
                .y(base_offset.1 + y * strength);
        }
        if let Some((size, fraction)) = self.grow {
            let start = (1.0 - (1.0 - fraction) * strength).clamp(0.2, 1.0);
            shown = shown.width(size).height(size);
            hidden = hidden.width(size * start).height(size * start);
        }
        if let Some(from) = self.scale_from {
            let start = (1.0 - (1.0 - from) * strength).clamp(0.2, 1.0);
            shown = shown.scale(1.0);
            hidden = hidden.scale(start);
        }

        (
            shown,
            hidden,
            self.blur_from.map(|radius| radius * strength),
        )
    }
}

impl Default for Entrance {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether an item is appearing or leaving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Transition {
    Enter,
    Exit,
}

fn apply_entrance<E: Styled + gpui::IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    entrance: Entrance,
    transition: Transition,
    config: &MotionConfig,
    base_offset: (f32, f32),
) -> gpui::AnyElement {
    let (shown, hidden, blur) = entrance.targets(config, base_offset);
    let (target, initial, delay, duration) = match transition {
        Transition::Enter => (
            shown,
            hidden,
            entrance.delay,
            entrance.resolved_duration(config),
        ),
        Transition::Exit => (
            hidden,
            shown,
            Duration::ZERO,
            entrance.exit_duration(config),
        ),
    };
    let motion = Motion::new(target)
        .initial(initial)
        .transition(entrance.curve.transition(duration))
        .delay(delay);
    let style_motion = blur.map(|blur| {
        let (target_blur, initial_blur) = match transition {
            Transition::Enter => (0.0, blur),
            Transition::Exit => (blur, 0.0),
        };
        StyleMotion::new(StyleTarget::default().value(StyleProperty::Blur, target_blur))
            .initial(StyleTarget::default().value(StyleProperty::Blur, initial_blur))
            .transition(entrance.curve.transition(duration))
            .delay(delay)
    });
    let element = MotionElement::new(id, element, motion);
    match style_motion {
        Some(style_motion) => element.style_motion(style_motion).into_any_element(),
        None => element.into_any_element(),
    }
}

/// GPUI element extension backed by retained fork MotionState.
pub(super) trait AnimateIn: Styled + gpui::IntoElement + Sized + 'static {
    fn animate_in(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &MotionConfig,
    ) -> gpui::AnyElement {
        apply_entrance(self, id, entrance, Transition::Enter, config, (0.0, 0.0))
    }

    #[allow(dead_code)]
    fn animate_in_with_offset(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &MotionConfig,
        base_offset: (f32, f32),
    ) -> gpui::AnyElement {
        self.animate_with_offset(id, entrance, Transition::Enter, config, base_offset)
    }

    fn animate_out(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &MotionConfig,
    ) -> gpui::AnyElement {
        self.animate_with_offset(id, entrance, Transition::Exit, config, (0.0, 0.0))
    }

    fn animate(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        transition: Transition,
        config: &MotionConfig,
    ) -> gpui::AnyElement {
        match transition {
            Transition::Enter => self.animate_in(id, entrance, config),
            Transition::Exit => self.animate_out(id, entrance, config),
        }
    }

    fn animate_with_offset(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        transition: Transition,
        config: &MotionConfig,
        base_offset: (f32, f32),
    ) -> gpui::AnyElement {
        let id: ElementId = id.into();
        let id = if transition == Transition::Exit {
            ElementId::from((id, "exit"))
        } else {
            id
        };
        apply_entrance(self, id, entrance, transition, config, base_offset)
    }
}

impl<E: Styled + gpui::IntoElement + Sized + 'static> AnimateIn for E {}

/// Spring parameters for shell-level animation adapters.
#[derive(Clone, Copy, Debug)]
pub(super) struct SpringSpec {
    pub config: SpringConfig,
    pub epsilon: f32,
}

/// Animates a scalar using the fork's retained spring channels.
pub(super) trait AnimateSpring: Styled + gpui::IntoElement + Sized + 'static {
    fn animate_spring(
        self,
        id: impl Into<ElementId>,
        from: f32,
        to: f32,
        spring: SpringSpec,
        apply: impl Fn(Self, f32) -> Self + 'static,
    ) -> impl gpui::IntoElement {
        let parameters = spring.config;
        let engine_spring = Spring::new(
            parameters.stiffness,
            parameters.damping.max(f32::MIN_POSITIVE),
            parameters.mass,
        )
        .expect("Photon spring parameters are positive")
        .rest(
            f64::from(spring.epsilon.max(f32::MIN_POSITIVE)),
            f64::from(spring.epsilon.max(f32::MIN_POSITIVE))
                * f64::from((parameters.stiffness / parameters.mass).sqrt()),
        )
        .expect("Photon spring settling tolerance is positive");
        MotionElement::new(
            id,
            self,
            Motion::new(Target::default().x(to))
                .initial(Target::default().x(from))
                .transition(gpui_motion::Transition::Spring(engine_spring)),
        )
        .map(move |element, frame: Frame| apply(element, frame.target().x_px()))
    }
}

impl<E: Styled + gpui::IntoElement + Sized + 'static> AnimateSpring for E {}

/// Animates a vertical position offset with a retained element motion channel.
pub(super) fn animate_vertical_position<E>(
    element: E,
    id: impl Into<ElementId>,
    from: f32,
    duration: Duration,
) -> gpui::AnyElement
where
    E: Styled + gpui::IntoElement + 'static,
{
    let motion = Motion::new(Target::default().y(0.0))
        .initial(Target::default().y(from))
        .transition(TweenCurve::EaseOut.transition(duration));
    MotionElement::new(id, element, motion)
        .map(|element: E, frame: Frame| element.top(gpui::px(frame.target().y_px())))
        .into_any_element()
}

/// A continuously retargetable scalar backed by the fork's analytic MotionValue.
#[derive(Clone, Debug)]
pub(super) struct Tween {
    value: std::cell::RefCell<MotionValue>,
    epoch: Instant,
    target: f32,
    curve: TweenCurve,
}

impl Tween {
    pub fn at(value: f32) -> Self {
        Self::at_with_curve(value, TweenCurve::default())
    }

    pub fn at_with_curve(value: f32, curve: TweenCurve) -> Self {
        Self {
            value: std::cell::RefCell::new(MotionValue::new(f64::from(value))),
            epoch: Instant::now(),
            target: value,
            curve,
        }
    }

    pub fn animate_to(&mut self, target: f32, speed: Speed, config: &MotionConfig) {
        self.animate_to_for(target, config.duration(speed), config);
    }

    pub fn animate_to_for(&mut self, target: f32, duration: Duration, config: &MotionConfig) {
        if !config.allows_motion() {
            self.jump_to(target);
        } else {
            self.animate_to_at(target, duration, self.curve, Instant::now());
        }
    }

    pub fn animate_to_at(
        &mut self,
        target: f32,
        duration: Duration,
        curve: TweenCurve,
        now: Instant,
    ) {
        if self.target == target && !duration.is_zero() {
            return;
        }
        self.target = target;
        self.curve = curve;
        self.value.borrow_mut().animate(
            f64::from(target),
            curve.transition(duration),
            now.saturating_duration_since(self.epoch),
        );
    }

    pub fn jump_to(&mut self, value: f32) {
        self.jump_to_at(value, Instant::now());
    }

    pub fn jump_to_at(&mut self, value: f32, now: Instant) {
        self.target = value;
        self.value
            .borrow_mut()
            .jump(f64::from(value), now.saturating_duration_since(self.epoch));
    }

    pub fn value_at(&self, now: Instant) -> f32 {
        self.value
            .borrow_mut()
            .sample(now.saturating_duration_since(self.epoch), false)
            .value() as f32
    }

    #[allow(dead_code)]
    pub fn value_with_config(&self, now: Instant, config: &MotionConfig) -> f32 {
        if config.allows_motion() {
            self.value_at(now)
        } else {
            self.target
        }
    }

    pub fn value(&self) -> f32 {
        self.value_at(Instant::now())
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn is_running_at(&self, now: Instant) -> bool {
        self.value
            .borrow_mut()
            .sample(now.saturating_duration_since(self.epoch), false)
            .is_animating()
    }

    #[allow(dead_code)]
    pub fn is_running_with_config(&self, now: Instant, config: &MotionConfig) -> bool {
        config.allows_motion() && self.is_running_at(now)
    }

    pub fn is_running(&self) -> bool {
        self.is_running_at(Instant::now())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[allow(dead_code)] // Matches the legacy TweenCurve options for downstream Photon UI.
pub(super) enum TweenCurve {
    Linear,
    EaseOut,
    SmoothStep,
    #[default]
    EaseInOutCubic,
    Spring(SpringConfig),
}

pub(super) const THEME_PICKER_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(360.0, 18.0, 1.0));

pub(super) const SIDEBAR_RESIZE_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(520.0, 30.0, 1.0));

impl TweenCurve {
    fn transition(self, duration: Duration) -> gpui_motion::Transition {
        let easing = match self {
            Self::Linear => Easing::Linear,
            Self::EaseOut => custom_ease_out(),
            Self::SmoothStep => Easing::EaseInOut,
            Self::EaseInOutCubic => custom_ease_in_out_cubic(),
            Self::Spring(config) => {
                return gpui_motion::Transition::Spring(
                    Spring::new(
                        config.stiffness,
                        config.damping.max(f32::MIN_POSITIVE),
                        config.mass,
                    )
                    .expect("Photon spring parameters are positive"),
                );
            }
        };
        if duration.is_zero() {
            return gpui_motion::Transition::Instant;
        }
        gpui_motion::Transition::Tween(duration, easing)
    }
}

/// Keeps one value mounted through its exit transition.
pub(super) struct Presence<T> {
    shown: Option<T>,
    leaving: Option<(T, Instant)>,
    expiry: Option<gpui::Task<()>>,
}

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self {
            shown: None,
            leaving: None,
            expiry: None,
        }
    }
}

impl<T: Clone + 'static> Presence<T> {
    pub fn sync<V: 'static>(
        &mut self,
        current: Option<T>,
        entrance: Entrance,
        config: &MotionConfig,
        cx: &mut gpui::Context<V>,
    ) -> Option<(T, Transition)> {
        let allowed = config.allows_motion() && !cx.reduce_motion();
        let duration = if allowed {
            entrance.exit_duration(config)
        } else {
            Duration::ZERO
        };
        let now = Instant::now();

        if let Some(current) = current {
            self.shown = Some(current.clone());
            self.leaving = None;
            self.expiry = None;
            return Some((current, Transition::Enter));
        }

        if !allowed {
            self.shown = None;
            self.leaving = None;
            self.expiry = None;
            return None;
        }

        if let Some(last) = self.shown.take() {
            if duration.is_zero() {
                self.leaving = None;
                return None;
            }
            self.leaving = Some((last.clone(), now));
            self.expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
            return Some((last, Transition::Exit));
        }

        match &self.leaving {
            Some((last, started)) if now.saturating_duration_since(*started) < duration => {
                Some((last.clone(), Transition::Exit))
            }
            Some(_) => {
                self.leaving = None;
                self.expiry = None;
                None
            }
            None => None,
        }
    }
}

/// Pixel distances selected for Photon's visual design.
pub(super) mod distance {
    pub const HINT: f32 = 4.0;
    pub const NUDGE: f32 = 8.0;
    pub const SHIFT: f32 = 12.0;
    pub const TRAVEL: f32 = 32.0;
}

/// Photon-specific control stretch tuning.
const ICON_BUTTON_PRESS_SCALE: f32 = 0.92;

fn press_spring() -> Spring {
    Spring::new(480.0, 44.0, 1.0).expect("Photon press spring parameters are positive")
}

pub(super) fn button_jiggle_spring() -> Spring {
    Spring::new(1_600.0, 80.0, 1.0).expect("Photon button jiggle spring parameters are positive")
}

/// The default spring for icons that move on hover, taken from
/// heroicons-animated's cog: it settles in under a second, overshooting its
/// target slightly.
pub(super) fn hover_icon_spring() -> Spring {
    Spring::new(50.0, 10.0, 1.0).expect("Photon hover icon spring parameters are positive")
}

fn press_scale<E: StatefulInteractiveElement + Styled + IntoElement + 'static>(
    mut element: E,
    scale: f32,
    focus_handle: Option<FocusHandle>,
) -> PressScaleElement<E> {
    let id = element
        .interactivity()
        .element_id
        .clone()
        .expect("pressable elements need a stable GPUI element ID");
    let press = gpui_motion::press_scale(id, element)
        .scale(scale)
        .spring(press_spring())
        .tab_index(None);
    if let Some(focus_handle) = focus_handle {
        press.focus_handle(focus_handle).preserve_focus_tab_stop()
    } else {
        press
    }
}

pub(super) fn pressable<E: StatefulInteractiveElement + Styled + IntoElement + 'static>(
    element: E,
) -> PressScaleElement<E> {
    press_scale(element, 0.97, None)
}

pub(super) fn pressable_with_focus<
    E: StatefulInteractiveElement + Styled + IntoElement + 'static,
>(
    element: E,
    focus_handle: FocusHandle,
) -> PressScaleElement<E> {
    press_scale(element, 0.97, Some(focus_handle))
}

pub(super) fn icon_pressable<E: StatefulInteractiveElement + Styled + IntoElement + 'static>(
    element: E,
) -> PressScaleElement<E> {
    press_scale(element, ICON_BUTTON_PRESS_SCALE, None)
}

pub(super) fn control_thumb_transition<E: Styled + IntoElement + 'static>(
    id: ElementId,
    element: E,
    offset: f32,
    width: f32,
    tracking: bool,
    config: &MotionConfig,
) -> impl IntoElement {
    MotionElement::new(
        id,
        element,
        Motion::new(Target::default().x(offset).width(width))
            .transition(control_transition(tracking, config)),
    )
}

fn control_transition(tracking: bool, config: &MotionConfig) -> gpui_motion::Transition {
    let duration = config.duration(Speed::Quick);
    if tracking || duration.is_zero() {
        gpui_motion::Transition::Instant
    } else {
        gpui_motion::Transition::Tween(duration, custom_ease_out())
    }
}

pub(super) fn mix(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

pub(super) const fn fade() -> Entrance {
    Entrance::new().fade_from(0.0).speed(Speed::Quick)
}

pub(super) const fn toast() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Bottom, distance::SHIFT)
        .scale_from(0.97)
        .blur_from(2.0)
        .speed(Speed::Gentle)
}

pub(super) const fn rise() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Bottom, distance::SHIFT)
}

pub(super) const fn fall() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Top, distance::NUDGE)
}

pub(super) const fn popover() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Top, distance::HINT)
        .speed(Speed::Quick)
}

pub(super) const fn slide_in(edge: Edge) -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(edge, distance::TRAVEL)
        .speed(Speed::Gentle)
}

pub(super) const fn pop(size: f32) -> Entrance {
    Entrance::new()
        .grow_from(size, 0.35)
        .curve(Curve::Overshoot)
        .speed(Speed::Standard)
}

#[cfg(test)]
mod translated_element_tests {
    use super::translated;
    use gpui::{
        InteractiveElement, Modifiers, MouseButton, ParentElement, ScaledPixels, Styled, div,
        point, px, size,
    };
    use std::{cell::Cell, rc::Rc};

    #[gpui::test]
    fn translated_elements_move_paint_and_hit_testing_without_layout_offsets(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let presses = Rc::new(Cell::new(0));
        let recorded_presses = presses.clone();

        cx.draw(
            point(px(0.), px(0.)),
            size(px(100.), px(100.)),
            move |_, _| {
                div().size_full().relative().child(translated(
                    div()
                        .id("translated-motion-test")
                        .absolute()
                        .left(px(10.))
                        .top(px(10.))
                        .size(px(20.))
                        .bg(gpui::blue())
                        .on_mouse_down(MouseButton::Left, move |_, _, _| {
                            recorded_presses.set(recorded_presses.get() + 1);
                        }),
                    point(px(30.), px(5.)),
                ))
            },
        );

        let scale = cx.update(|window, _| window.scale_factor());
        let quads = cx.update(|window, _| window.painted_quads());
        let expected_origin = point(ScaledPixels(40. * scale), ScaledPixels(15. * scale));
        let expected_size = size(ScaledPixels(20. * scale), ScaledPixels(20. * scale));
        assert!(
            quads.iter().any(|quad| {
                quad.bounds.origin == expected_origin && quad.bounds.size == expected_size
            }),
            "the translated element's painted quad should use its visual position"
        );

        cx.simulate_click(point(px(15.), px(15.)), Modifiers::none());
        assert_eq!(
            presses.get(),
            0,
            "the original layout position must not hit"
        );
        cx.simulate_click(point(px(45.), px(20.)), Modifiers::none());
        assert_eq!(presses.get(), 1, "the translated visual position must hit");
    }
}
