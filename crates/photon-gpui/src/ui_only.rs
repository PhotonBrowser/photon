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

pub fn ensure_linked() {}

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
        custom_element_surface(root, &context)
            .child(gpui::div().absolute().size_full().bg(gpui::rgb(0xffffff)))
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
