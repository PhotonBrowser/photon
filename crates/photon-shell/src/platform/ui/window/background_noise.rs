//! A reusable static grain overlay for the translucent window background.

use gpui::{ImageSource, ObjectFit, RenderImage, img, prelude::*};
use image::{Frame, RgbaImage};
use std::sync::{Arc, OnceLock};

const TEXTURE_SIZE: u32 = 1_024;
const MAX_NOISE_OPACITY: f32 = 0.16;
const TEXTURE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/background-noise.rgba"));

pub(super) fn overlay(intensity: u8) -> impl IntoElement {
    let opacity = f32::from(intensity.min(photon_core::BACKGROUND_NOISE_INTENSITY_MAX))
        / f32::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX);
    img(ImageSource::Render(texture()))
        .absolute()
        .inset_0()
        .size_full()
        .object_fit(ObjectFit::Cover)
        .opacity(opacity * MAX_NOISE_OPACITY)
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
