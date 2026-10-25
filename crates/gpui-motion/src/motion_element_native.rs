use crate::{
    Drag, Frame, GestureState, GestureTargets, Motion, MotionState, Spring, StyleFrame,
    StyleMotion, StyleMotionState, Target, Transition,
};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, Empty, FocusHandle, GlobalElementId,
    InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId, ParentElement,
    StatefulInteractiveElement, StyleRefinement, Styled, Window,
};

/// Applies a sampled motion frame to an element.
pub trait MotionApply<E> {
    /// Applies the sampled values for this frame.
    fn apply(&mut self, element: E, frame: Frame) -> E;
}

/// The default MotionApply implementation, which uses Frame::apply.
#[doc(hidden)]
pub struct DefaultMotionApply;

impl<E: Styled> MotionApply<E> for DefaultMotionApply {
    fn apply(&mut self, element: E, frame: Frame) -> E {
        frame.apply(element)
    }
}

impl<E, F: FnMut(E, Frame) -> E> MotionApply<E> for F {
    fn apply(&mut self, element: E, frame: Frame) -> E {
        self(element, frame)
    }
}

/// A styled element animated by a retained, ID-scoped MotionState.
///
/// State is retained by GPUI's keyed element state, so callers can declare
/// targets each render without owning animation tasks. Keep id stable for the
/// logical element. Use map to apply a frame to custom values and style_motion
/// for independent style properties.
pub struct MotionElement<E, F = DefaultMotionApply> {
    id: ElementId,
    element: Option<E>,
    motion: Motion,
    style_motion: Option<StyleMotion>,
    apply: F,
}

/// A button-like element with an opt-in animated scale while pressed.
///
/// The gesture state is retained by the stable element ID, so the caller does
/// not need to keep a separate [`GestureState`] for simple press feedback. The
/// child retains its role, label, click handlers, and other accessibility
/// metadata. Its own focus handling is tracked through the wrapper.
pub struct PressScaleElement<E> {
    id: ElementId,
    element: Option<E>,
    scale: f32,
    spring: Spring,
    focus_handle: Option<FocusHandle>,
    gesture_state: Option<GestureState>,
    tab_index: Option<isize>,
    enabled: bool,
    preserve_focus_tab_stop: bool,
    focus_tab_stop: Option<bool>,
    jiggle: Option<PressJiggle>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PressJiggle {
    pub(crate) range: f32,
    pub(crate) resistance: f32,
    pub(crate) spring: Spring,
}

impl<E> PressScaleElement<E> {
    /// Sets the painted scale used while the pointer or activation key is held.
    /// Scale is centered and does not change layout or hit-test bounds.
    pub fn scale(mut self, value: f32) -> Self {
        assert!(
            value.is_finite() && value >= 0.0,
            "scale must be finite and non-negative"
        );
        self.scale = value;
        self
    }

    /// Sets the spring used when pressing and releasing the element.
    pub fn spring(mut self, value: Spring) -> Self {
        self.spring = value;
        self
    }

    /// Adds a small, resistant drag while the pointer is held and moved.
    /// The element springs back to its original position on release.
    pub fn jiggle(mut self, range: f32, resistance: f32, spring: Spring) -> Self {
        assert!(
            range.is_finite() && range > 0.0 && (0.0..=1.0).contains(&resistance),
            "invalid press jiggle parameters"
        );
        self.jiggle = Some(PressJiggle {
            range,
            resistance,
            spring,
        });
        self
    }

    /// Reuses a control's existing focus handle, preserving application focus ownership.
    pub fn focus_handle(mut self, value: FocusHandle) -> Self {
        self.focus_handle = Some(value);
        self
    }

    /// Uses caller-owned gesture state for controls that need to inspect their
    /// interaction or share it with another native gesture binding.
    pub fn gesture_state(mut self, value: GestureState) -> Self {
        self.gesture_state = Some(value);
        self
    }

    /// Preserves the tab-stop policy already configured on the supplied focus handle.
    pub fn preserve_focus_tab_stop(mut self) -> Self {
        self.preserve_focus_tab_stop = true;
        self
    }

    /// Sets the focus handle's tab-stop policy when it differs from `enabled`.
    pub fn focus_tab_stop(mut self, value: Option<bool>) -> Self {
        self.focus_tab_stop = value;
        self
    }

    /// Sets the child's tab index. Defaults to zero for a standard button-like control.
    pub fn tab_index(mut self, value: Option<isize>) -> Self {
        self.tab_index = value;
        self
    }

    /// Enables or disables the wrapped control and its press handlers.
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}

/// Adds opt-in spring scale feedback to an accessible native control.
///
/// Keep `id` stable for the logical control and call this after configuring the
/// child. Native press, outside release, keyboard activation, cancellation,
/// focus, and reduced-motion handling are supplied by the gesture adapter.
///
/// ```
/// use gpui::{div, prelude::*, Role};
/// use gpui_ce_motion::{press_scale, Spring};
/// use std::time::Duration;
///
/// let button = press_scale("save-button", div().role(Role::Button).child("Save"))
///     .scale(0.97)
///     .spring(Spring::new(480.0, 44.0, 1.0).unwrap());
/// ```
pub fn press_scale<E>(id: impl Into<ElementId>, element: E) -> PressScaleElement<E>
where
    E: IntoElement + StatefulInteractiveElement + Styled + 'static,
{
    PressScaleElement {
        id: id.into(),
        element: Some(element),
        scale: 0.97,
        spring: Spring::new(480.0, 44.0, 1.0).expect("valid default press spring"),
        focus_handle: None,
        gesture_state: None,
        tab_index: Some(0),
        enabled: true,
        preserve_focus_tab_stop: false,
        focus_tab_stop: None,
        jiggle: None,
    }
}

impl<E: Styled + IntoElement + 'static> MotionElement<E> {
    /// Wraps a styled element with a declarative motion target.
    pub fn new(id: impl Into<ElementId>, element: E, motion: Motion) -> Self {
        Self {
            id: id.into(),
            element: Some(element),
            motion,
            style_motion: None,
            apply: DefaultMotionApply,
        }
    }
}

impl<E, F> MotionElement<E, F> {
    /// Applies a custom presentation mapping for each sampled frame.
    pub fn map<G: MotionApply<E>>(self, apply: G) -> MotionElement<E, G> {
        MotionElement {
            id: self.id,
            element: self.element,
            motion: self.motion,
            style_motion: self.style_motion,
            apply,
        }
    }

    /// Adds independently retained style motion to this element.
    pub fn style_motion(mut self, motion: StyleMotion) -> Self {
        self.style_motion = Some(motion);
        self
    }
}

#[derive(Default)]
struct RetainedMotion {
    motion: MotionState,
    style: Option<StyleMotionState>,
}

impl<E, F> IntoElement for MotionElement<E, F>
where
    E: Styled + IntoElement + 'static,
    F: MotionApply<E> + 'static,
{
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E, F> Element for MotionElement<E, F>
where
    E: Styled + IntoElement + 'static,
    F: MotionApply<E> + 'static,
{
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let (frame, style_frame): (Frame, Option<StyleFrame>) = window.with_element_state(
            id.expect("motion element has an ID"),
            |state: Option<RetainedMotion>, window| {
                let mut state = state.unwrap_or_default();
                let frame = state.motion.frame(self.motion, window, cx);
                let style_frame = self.style_motion.map(|motion| {
                    state
                        .style
                        .get_or_insert_with(StyleMotionState::default)
                        .frame(motion, window, cx)
                });
                ((frame, style_frame), state)
            },
        );

        let child = self.element.take().expect("motion layout requested once");
        let mut child = if frame.is_mounted() {
            let child = self.apply.apply(child, frame);
            match style_frame {
                Some(style) => style.apply(child).into_any_element(),
                None => child.into_any_element(),
            }
        } else {
            Empty.into_any_element()
        };

        let layout = child.request_layout(window, cx);
        (layout, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

impl<E: IntoElement + StatefulInteractiveElement + Styled + 'static> IntoElement
    for PressScaleElement<E>
{
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: IntoElement + StatefulInteractiveElement + Styled + 'static> Element
    for PressScaleElement<E>
{
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let motion =
            Motion::new(Target::default().scale(1.0)).transition(Transition::Spring(self.spring));
        let targets = GestureTargets::default().while_press(Target::default().scale(self.scale));
        let mut child = window.with_element_state(
            id.expect("press scale element has an ID"),
            |state: Option<GestureState>, _| {
                let state = state.unwrap_or_else(|| self.gesture_state.clone().unwrap_or_default());
                let state = if let Some(jiggle) = self.jiggle {
                    state.with_drag_if_absent(
                        Drag::new([0.0; 2], [0.0; 2], [0.0; 2])
                            .elasticity(1.0)
                            .momentum(false)
                            .spring(jiggle.spring),
                    )
                } else {
                    state
                };
                let element = self
                    .element
                    .take()
                    .expect("press scale layout requested once")
                    .disabled(!self.enabled);
                let gesture = state
                    .bind(self.id.clone(), element, motion, targets)
                    .capture_press()
                    .preserve_position()
                    .jiggle(self.jiggle)
                    .tab_index(self.tab_index)
                    .focus_tab_stop(self.focus_tab_stop);
                let gesture = if let Some(focus) = self.focus_handle.clone() {
                    gesture.focus_handle(focus)
                } else {
                    gesture
                };
                let gesture = if self.preserve_focus_tab_stop {
                    gesture.preserve_focus_tab_stop()
                } else {
                    gesture
                };
                let child = gesture.enabled(self.enabled).into_any_element();
                (child, state)
            },
        );

        let layout = child.request_layout(window, cx);

        (layout, child)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

impl<E: Styled> Styled for PressScaleElement<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element
            .as_mut()
            .expect("press scale element is configured before rendering")
            .style()
    }
}

impl<E: InteractiveElement> InteractiveElement for PressScaleElement<E> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.element
            .as_mut()
            .expect("press scale element is configured before rendering")
            .interactivity()
    }

    fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle.clone());
        self.preserve_focus_tab_stop = true;
        self.element = Some(
            self.element
                .take()
                .expect("press scale element is configured before rendering")
                .track_focus(focus_handle),
        );
        self
    }

    fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = Some(index);
        self.element = Some(
            self.element
                .take()
                .expect("press scale element is configured before rendering")
                .tab_index(index),
        );
        self
    }

    fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.focus_tab_stop = Some(tab_stop);
        self.element = Some(
            self.element
                .take()
                .expect("press scale element is configured before rendering")
                .tab_stop(tab_stop),
        );
        self
    }
}

impl<E: StatefulInteractiveElement> StatefulInteractiveElement for PressScaleElement<E> {}

impl<E: ParentElement> ParentElement for PressScaleElement<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.element
            .as_mut()
            .expect("press scale element is configured before rendering")
            .extend(elements);
    }
}
