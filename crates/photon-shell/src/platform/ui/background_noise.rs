//! A reusable static grain overlay for the translucent window background.

use gpui::{ImageSource, ObjectFit, RenderImage, img, prelude::*, px};
use image::{Frame, RgbaImage};
use std::sync::{Arc, OnceLock};

const TEXTURE_SIZE: u32 = 1_024;
const TEXTURE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/background-noise.rgba"));

/// Grain clipped to `radius`, so it follows the rounded surface it covers.
pub(super) fn overlay(intensity: u8, radius: f32) -> impl IntoElement {
    let opacity = f32::from(intensity.min(photon_core::BACKGROUND_NOISE_INTENSITY_MAX))
        / f32::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX);
    img(ImageSource::Render(texture()))
        .absolute()
        .inset_0()
        .size_full()
        .rounded(px(radius))
        .object_fit(ObjectFit::Cover)
        .opacity(
            opacity
                * crate::platform::theme_config::theme()
                    .opacity
                    .background_noise_max,
        )
}

fn texture() -> Arc<RenderImage> {
    static TEXTURE: OnceLock<Arc<RenderImage>> = OnceLock::new();

    TEXTURE
        .get_or_init(|| {
            let pixels = RgbaImage::from_raw(TEXTURE_SIZE, TEXTURE_SIZE, TEXTURE_BYTES.to_vec())
                .expect("precomputed background noise texture has the expected dimensions");
            Arc::new(RenderImage::new([Frame::new(pixels)]))
        })
        .clone()
}
