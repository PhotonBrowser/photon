//! A reusable static grain overlay for the translucent window background.

use gpui::{ImageSource, ObjectFit, RenderImage, img, prelude::*};
use image::{Frame, Rgba, RgbaImage};
use std::sync::{Arc, OnceLock};

const TEXTURE_SIZE: u32 = 1_024;
const MAX_NOISE_OPACITY: f32 = 0.16;

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
            let pixels = RgbaImage::from_fn(TEXTURE_SIZE, TEXTURE_SIZE, |x, y| {
                let mut value = x
                    .wrapping_mul(0x9e37_79b9)
                    .wrapping_add(y.wrapping_mul(0x85eb_ca6b))
                    .wrapping_add(0xc2b2_ae35);
                value ^= value >> 16;
                value = value.wrapping_mul(0x7feb_352d);
                value ^= value >> 15;
                let grain = (value >> 24) as u8;
                Rgba([grain, grain, grain, u8::MAX])
            });
            Arc::new(RenderImage::new([Frame::new(pixels)]))
        })
        .clone()
}
