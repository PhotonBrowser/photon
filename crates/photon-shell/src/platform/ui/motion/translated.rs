//! An element moved after layout, painted and hit-tested where it appears.

use super::*;

/// Moves an element's painted and interactive bounds without changing layout.
pub(in super::super) fn translated<E: IntoElement>(
    element: E,
    offset: Point<Pixels>,
) -> TranslatedElement<E::Element> {
    TranslatedElement {
        element: element.into_element(),
        offset,
    }
}

/// Applies a frame-local offset after layout, including to descendant hitboxes.
pub(in super::super) struct TranslatedElement<E> {
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
