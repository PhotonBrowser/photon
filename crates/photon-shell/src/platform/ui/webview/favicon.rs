//! The page's icon, shown in its tab.

use gpui::{Context, RenderImage};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use super::PhotonWebView;

/// A page icon and a key identifying its pixels.
#[derive(Clone)]
pub(in crate::platform) struct Favicon {
    pub(in crate::platform) image: Arc<RenderImage>,
    /// Equal for identical icons, which Engine may report more than once.
    pub(in crate::platform) key: u64,
}

impl Favicon {
    /// Wraps straight-alpha BGRA pixels, the layout GPUI images hold.
    pub(in crate::platform) fn from_bgra(pixels: &[u8], width: u32, height: u32) -> Option<Self> {
        let mut hasher = DefaultHasher::new();
        (width, height, pixels).hash(&mut hasher);
        let buffer = image::RgbaImage::from_raw(width, height, pixels.to_vec())?;
        Some(Self {
            image: Arc::new(RenderImage::new([image::Frame::new(buffer)])),
            key: hasher.finish(),
        })
    }
}

impl PhotonWebView {
    pub(in crate::platform) fn set_favicon(
        &mut self,
        favicon: Option<Favicon>,
        cx: &mut Context<Self>,
    ) {
        let key = |favicon: &Option<Favicon>| favicon.as_ref().map(|favicon| favicon.key);
        if key(&favicon) == key(&self.favicon) {
            return;
        }
        if let Some(replaced) = std::mem::replace(&mut self.favicon, favicon) {
            cx.drop_image(replaced.image, None);
        }
        self.state_changed(cx);
    }
}
