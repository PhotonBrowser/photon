//! App-owned titlebar input behavior shared by the engine and UI-only shells.

use gpuix_native::{
    CustomElement, CustomElementFactory, CustomRenderContext, custom_element_surface,
};

struct TitlebarDragRegion;

struct TitlebarDragRegionFactory;

impl CustomElementFactory for TitlebarDragRegionFactory {
    fn element_type(&self) -> &str {
        "photon-titlebar-drag-region"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(TitlebarDragRegion)
    }
}

gpuix_native::register_custom_element!(|| Box::new(TitlebarDragRegionFactory));

pub(super) fn ensure_linked() {}

impl CustomElement for TitlebarDragRegion {
    fn render(
        &mut self,
        context: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<gpuix_native::GpuixView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let root = gpui::div()
            .id(gpui::SharedString::from(format!(
                "photon-titlebar-drag-region-{}",
                context.id()
            )))
            .size_full();
        custom_element_surface(root, &context)
            .on_mouse_down(gpui::MouseButton::Left, |event, window, _cx| {
                if event.click_count >= 2 {
                    window.titlebar_double_click();
                } else {
                    window.start_window_move();
                }
            })
            .into_any_element()
    }

    fn set_prop(&mut self, _key: &str, _value: serde_json::Value) {}

    fn supported_props(&self) -> &'static [&'static str] {
        &[]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &[]
    }

    fn destroy(&mut self) {}
}
