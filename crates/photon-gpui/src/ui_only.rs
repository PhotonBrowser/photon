//! GPUI web surface used to develop the shell without starting Photon Engine.

use gpui::SharedString;
use gpuix_native::{
    CustomElement, CustomElementFactory, CustomRenderContext, custom_element_surface,
};

pub struct PhotonWebViewElement;

pub struct PhotonWebViewFactory;

impl CustomElementFactory for PhotonWebViewFactory {
    fn element_type(&self) -> &str {
        "photon-webview"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(PhotonWebViewElement)
    }
}

gpuix_native::register_custom_element!(|| Box::new(PhotonWebViewFactory));

pub fn ensure_linked() {
    super::titlebar::ensure_linked();
}

impl CustomElement for PhotonWebViewElement {
    fn render(
        &mut self,
        context: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<gpuix_native::GpuixView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let root = gpui::div()
            .id(SharedString::from(format!(
                "photon-webview-{}",
                context.id()
            )))
            .size_full();
        let mut surface = gpui::div().absolute().size_full().bg(gpui::rgb(0xffffff));
        if let Some(style) = context.style() {
            if let Some(radius) = style.border_radius {
                surface = surface.rounded(gpui::px(radius as f32));
            }
            if let Some(radius) = style.border_top_left_radius {
                surface = surface.rounded_tl(gpui::px(radius as f32));
            }
            if let Some(radius) = style.border_top_right_radius {
                surface = surface.rounded_tr(gpui::px(radius as f32));
            }
            if let Some(radius) = style.border_bottom_left_radius {
                surface = surface.rounded_bl(gpui::px(radius as f32));
            }
            if let Some(radius) = style.border_bottom_right_radius {
                surface = surface.rounded_br(gpui::px(radius as f32));
            }
        }
        custom_element_surface(root, &context)
            .child(surface)
            .into_any_element()
    }

    fn set_prop(&mut self, _key: &str, _value: serde_json::Value) {}

    fn supported_props(&self) -> &'static [&'static str] {
        &["url"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &[]
    }

    fn destroy(&mut self) {}

    fn needs_polling(&self) -> bool {
        false
    }

    fn poll(&mut self) -> bool {
        false
    }
}
