//! Entrances and exits: how an element appears and leaves, and the presets
//! Photon's surfaces share.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Photon-specific composition of entrance and exit effects.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Entrance {
    speed: Speed,
    duration_override: Option<Duration>,
    delay: Duration,
    curve: Curve,
    exit_curve: Option<Curve>,
    opacity_from: Option<f32>,
    offset_from: Option<(f32, f32)>,
    blur_from: Option<f32>,
    grow: Option<(f32, f32)>,
    scale_from: Option<f32>,
    /// The edge an element scales from, and its length across that edge.
    scale_origin: Option<(Edge, f32)>,
}

impl Entrance {
    pub const fn new() -> Self {
        Self {
            speed: Speed::Standard,
            duration_override: None,
            delay: Duration::ZERO,
            curve: Curve::EaseOut,
            exit_curve: None,
            opacity_from: None,
            offset_from: None,
            blur_from: None,
            grow: None,
            scale_from: None,
            scale_origin: None,
        }
    }

    pub const fn scale_from(mut self, scale: f32) -> Self {
        self.scale_from = Some(scale);
        self
    }

    /// Scales up from `scale` as though growing out of `edge`, as a
    /// popover grows from the side it opens on. `length` is the element's
    /// size across that edge: its width for a side, its height for the top or
    /// bottom.
    pub const fn scale_from_edge(mut self, scale: f32, edge: Edge, length: f32) -> Self {
        self.scale_from = Some(scale);
        self.scale_origin = Some((edge, length));
        self
    }

    /// The part a native view takes: the scale, about its centre. A native
    /// view moves with its parent's layout, which carries the slide and the
    /// shift toward the edge it grows from, and it does not fade or blur.
    pub(in super::super) const fn native_part(self) -> Self {
        let mut native = Self::new();
        native.speed = self.speed;
        native.duration_override = self.duration_override;
        native.delay = self.delay;
        native.curve = self.curve;
        native.exit_curve = self.exit_curve;
        native.scale_from = self.scale_from;
        native
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

    pub const fn exit_curve(mut self, curve: Curve) -> Self {
        self.exit_curve = Some(curve);
        self
    }

    fn resolved_duration(self, config: &MotionConfig) -> Duration {
        if !config.allows_motion() {
            return Duration::ZERO;
        }
        self.duration_override
            .unwrap_or_else(|| config.duration(self.speed))
    }

    pub(in super::super) fn exit_duration(self, config: &MotionConfig) -> Duration {
        self.resolved_duration(config) * 3 / 4
    }

    pub(in super::super) fn total_duration(self, config: &MotionConfig) -> Duration {
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
        let mut hidden_x = base_offset.0;
        let mut hidden_y = base_offset.1;

        if let Some(opacity) = self.opacity_from {
            shown = shown.opacity(1.0);
            hidden = hidden.opacity(opacity);
        }
        if let Some((x, y)) = self.offset_from {
            shown = shown.x(base_offset.0).y(base_offset.1);
            hidden_x = base_offset.0 + x * strength;
            hidden_y = base_offset.1 + y * strength;
            hidden = hidden.x(hidden_x).y(hidden_y);
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
            // Scaling about the centre, shift toward the edge so that edge
            // stays put.
            if let Some((edge, length)) = self.scale_origin {
                let shift = (1.0 - start) * length * 0.5;
                match edge {
                    Edge::Left => hidden_x -= shift,
                    Edge::Right => hidden_x += shift,
                    Edge::Top => hidden_y -= shift,
                    Edge::Bottom => hidden_y += shift,
                }
                shown = shown.x(base_offset.0).y(base_offset.1);
                hidden = hidden.x(hidden_x).y(hidden_y);
            }
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

/// The theme picker grows out of `edge`, the side it opens on, as a popover
/// does, while its controls fade and sharpen inside its glass. `width` is
/// the picker's width.
pub(in super::super) fn theme_picker_entrance(edge: Edge, width: f32) -> Entrance {
    let tokens = &crate::platform::theme_config::theme().motion.theme_picker;
    Entrance::new()
        .fade_from(0.0)
        .blur_from(tokens.blur_from)
        .scale_from_edge(tokens.scale_from, edge, width)
        .curve(Curve::EaseOut)
        .exit_curve(Curve::EaseInOut)
        .duration(Duration::from_millis(tokens.duration_ms))
}

/// Whether an item is appearing or leaving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Transition {
    Enter,
    Exit,
}

pub(super) fn apply_entrance<E: Styled + gpui::IntoElement + 'static>(
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
    let curve = match transition {
        Transition::Enter => entrance.curve,
        // A spring takes as long as it takes, but an exit must finish within
        // the time `Presence` keeps the element, so springs leave easing out.
        Transition::Exit => entrance.exit_curve.unwrap_or(match entrance.curve {
            Curve::Spring(_) => Curve::EaseOut,
            curve => curve,
        }),
    };
    let motion = Motion::new(target)
        .initial(initial)
        .transition(curve.transition(duration))
        .delay(delay);
    let style_motion = blur.map(|blur| {
        let (target_blur, initial_blur) = match transition {
            Transition::Enter => (0.0, blur),
            Transition::Exit => (blur, 0.0),
        };
        StyleMotion::new(StyleTarget::default().value(StyleProperty::Blur, target_blur))
            .initial(StyleTarget::default().value(StyleProperty::Blur, initial_blur))
            .transition(curve.transition(duration))
            .delay(delay)
    });
    let element = MotionElement::new(id, element, motion);
    match style_motion {
        Some(style_motion) => element.style_motion(style_motion).into_any_element(),
        None => element.into_any_element(),
    }
}

/// GPUI element extension backed by retained fork MotionState.
pub(in super::super) trait AnimateIn:
    Styled + gpui::IntoElement + Sized + 'static
{
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

    /// Reuses the same motion ID on enter and exit so keyed child state keeps its identity.
    fn animate_with_offset_stable_id(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        transition: Transition,
        config: &MotionConfig,
        base_offset: (f32, f32),
    ) -> gpui::AnyElement {
        apply_entrance(self, id, entrance, transition, config, base_offset)
    }
}

impl<E: Styled + gpui::IntoElement + Sized + 'static> AnimateIn for E {}

pub(in super::super) const fn fade() -> Entrance {
    Entrance::new().fade_from(0.0).speed(Speed::Quick)
}

pub(in super::super) const fn toast() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Bottom, distance::SHIFT)
        .scale_from(0.96)
        .blur_from(4.0)
        .curve(Curve::SMOOTH)
        .speed(Speed::Gentle)
}

pub(in super::super) const fn fall() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Top, distance::NUDGE)
        .curve(Curve::SNAPPY)
}

/// Menus and popovers grow out of what opened them, softly in focus.
pub(in super::super) const fn popover() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .scale_from(0.95)
        .slide_from(Edge::Top, distance::NUDGE / 2.0)
        .blur_from(2.0)
        .curve(Curve::SNAPPY)
        .speed(Speed::Quick)
}

pub(in super::super) const fn slide_in(edge: Edge) -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(edge, distance::TRAVEL)
        .blur_from(3.0)
        .curve(Curve::SMOOTH)
        .speed(Speed::Gentle)
}

pub(in super::super) const fn pop(size: f32) -> Entrance {
    Entrance::new()
        .grow_from(size, 0.35)
        .curve(Curve::Overshoot)
        .speed(Speed::Standard)
}
