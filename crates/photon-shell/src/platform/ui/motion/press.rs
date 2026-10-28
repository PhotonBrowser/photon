//! Feedback under the pointer: pressed controls, springing values and
//! surfaces fading between their colours.

use super::*;

/// Spring parameters for shell-level animation adapters.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct SpringSpec {
    pub config: SpringConfig,
    pub epsilon: f32,
}

/// Animates a scalar using the fork's retained spring channels.
pub(in super::super) trait AnimateSpring:
    Styled + gpui::IntoElement + Sized + 'static
{
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

/// How long a surface takes to change colour, as when hovered, pressed or
/// selected. Colour changes stay under reduced motion: they move nothing.
pub(super) const SURFACE_FADE: Duration = Duration::from_millis(140);

/// Fades a surface between its resting, hovered, pressed and selected
/// colours instead of switching them at once.
pub(in super::super) trait SurfaceFade: gpui::InteractiveElement + Sized {
    #[track_caller]
    fn surface_fade(self) -> Self {
        self.transitions(|transitions| transitions.bg(SURFACE_FADE).border_color(SURFACE_FADE))
    }
}

impl<E: gpui::InteractiveElement> SurfaceFade for E {}

/// Photon-specific control stretch tuning.
pub(super) const ICON_BUTTON_PRESS_SCALE: f32 = 0.92;

/// Underdamped, so a released control springs back with a little life.
pub(super) fn press_spring() -> Spring {
    Spring::new(640.0, 34.0, 1.0).expect("Photon press spring parameters are positive")
}

pub(in super::super) fn button_jiggle_spring() -> Spring {
    Spring::new(1_600.0, 80.0, 1.0).expect("Photon button jiggle spring parameters are positive")
}

/// The default spring for icons that move on hover, taken from
/// heroicons-animated's cog: it settles in under a second, overshooting its
/// target slightly.
pub(in super::super) fn hover_icon_spring() -> Spring {
    Spring::new(50.0, 10.0, 1.0).expect("Photon hover icon spring parameters are positive")
}

pub(super) fn press_scale<E: StatefulInteractiveElement + Styled + IntoElement + 'static>(
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

pub(in super::super) fn pressable<
    E: StatefulInteractiveElement + Styled + IntoElement + 'static,
>(
    element: E,
) -> PressScaleElement<E> {
    press_scale(element, 0.97, None)
}

pub(in super::super) fn pressable_with_focus<
    E: StatefulInteractiveElement + Styled + IntoElement + 'static,
>(
    element: E,
    focus_handle: FocusHandle,
) -> PressScaleElement<E> {
    press_scale(element, 0.97, Some(focus_handle))
}

pub(in super::super) fn icon_pressable<
    E: StatefulInteractiveElement + Styled + IntoElement + 'static,
>(
    element: E,
) -> PressScaleElement<E> {
    press_scale(element, ICON_BUTTON_PRESS_SCALE, None)
}

pub(in super::super) fn control_thumb_transition<E: Styled + IntoElement + 'static>(
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

pub(super) fn control_transition(tracking: bool, config: &MotionConfig) -> gpui_motion::Transition {
    let duration = config.duration(Speed::Quick);
    if tracking || duration.is_zero() {
        gpui_motion::Transition::Instant
    } else {
        gpui_motion::Transition::Tween(duration, custom_ease_out())
    }
}
