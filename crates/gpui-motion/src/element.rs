use crate::{Curve, Entrance, MotionConfig, Speed, Transition};
use gpui::{
    AnimationElement, AnimationExt, ElementId, IntoElement, Motion, SpringConfig,
    StatefulInteractiveElement, Styled,
};
use std::time::Duration;

/// Animates any styled element in or out with an [`Entrance`].
pub trait AnimateIn: Styled + IntoElement + Sized + 'static {
    /// Plays `entrance` the first time the element is shown under `id`.
    /// A new `id` plays it again.
    fn animate_in(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &MotionConfig,
    ) -> AnimationElement<Self> {
        let config = *config;
        self.with_animation(id, entrance.animation(&config), move |element, progress| {
            entrance.apply(element, progress, &config)
        })
    }

    /// Plays an entrance while preserving an element's existing relative
    /// offset, such as the temporary displacement used during tab reordering.
    fn animate_in_with_offset(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &MotionConfig,
        base_offset: (f32, f32),
    ) -> AnimationElement<Self> {
        let config = *config;
        self.with_animation(id, entrance.animation(&config), move |element, progress| {
            entrance.apply_with_offset(element, progress, &config, base_offset)
        })
    }

    /// Plays `entrance` backwards, so the element leaves the way it came.
    /// Use an `id` distinct from the entrance's, so the exit starts afresh.
    fn animate_out(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        config: &MotionConfig,
    ) -> AnimationElement<Self> {
        let config = *config;
        self.with_animation(
            id,
            entrance.exit_animation(&config),
            move |element, progress| entrance.apply(element, 1.0 - progress, &config),
        )
    }

    /// Animates in or out by `transition`, under an id for each.
    fn animate(
        self,
        id: impl Into<ElementId>,
        entrance: Entrance,
        transition: Transition,
        config: &MotionConfig,
    ) -> AnimationElement<Self> {
        let id = ElementId::from((id.into(), transition.name()));
        match transition {
            Transition::Enter => self.animate_in(id, entrance, config),
            Transition::Exit => self.animate_out(id, entrance, config),
        }
    }
}

impl<E: Styled + IntoElement + 'static> AnimateIn for E {}

/// Parameters for a scale response while a pointer or touch press is active.
#[derive(Clone, Copy, Debug)]
pub struct PressSpec {
    /// Scale applied while the element is pressed.
    pub scale: f32,
    /// Spring used to animate the scale transition.
    pub spring: SpringConfig,
}

/// Adds a configurable scale response while a pointer or touch press is active.
pub fn pressable<E: StatefulInteractiveElement + Styled>(element: E, spec: PressSpec) -> E {
    element
        .transitions(|transitions| transitions.scale(Motion::spring(spec.spring)))
        .active(|style| style.scale(spec.scale))
}

/// Adds a spring scale transition to a stateful interactive element.
pub fn scale_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    spring: SpringConfig,
) -> E {
    element.transitions(|transitions| transitions.scale(Motion::spring(spring)))
}

fn control_stretch_motion(config: &MotionConfig) -> Motion {
    Curve::EaseOut.motion(config.duration(Speed::Quick))
}

/// Transitions a control's scale and horizontal position using the quick timing tier.
pub fn control_scale_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    tracking: bool,
    config: &MotionConfig,
) -> E {
    let motion = if tracking {
        Motion::new(Duration::ZERO)
    } else {
        control_stretch_motion(config)
    };
    element.transitions(|transitions| transitions.scale(motion.clone()).left(motion))
}

/// Transitions a control's horizontal position using the quick timing tier.
pub fn control_left_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    config: &MotionConfig,
) -> E {
    element.transitions(|transitions| transitions.left(control_stretch_motion(config)))
}

/// Transitions a reusable thumb's width and horizontal position.
pub fn control_thumb_transition<E: StatefulInteractiveElement + Styled>(
    element: E,
    config: &MotionConfig,
) -> E {
    control_thumb_transition_tracking(element, false, config)
}

/// Transitions a reusable thumb while keeping tracked pointer movement immediate.
pub fn control_thumb_transition_tracking<E: StatefulInteractiveElement + Styled>(
    element: E,
    tracking: bool,
    config: &MotionConfig,
) -> E {
    let motion = if tracking {
        Motion::new(Duration::ZERO)
    } else {
        control_stretch_motion(config)
    };
    element.transitions(|transitions| transitions.w(motion.clone()).left(motion))
}
