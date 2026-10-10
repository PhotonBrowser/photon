//! Icons of visited pages, one small file per page address.

use std::path::{Path, PathBuf};

/// An icon's straight-alpha pixels in the order the shell draws them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaviconPixels {
    pub width: u32,
    pub height: u32,
    /// Four bytes per pixel, rows tightly packed.
    pub pixels: Vec<u8>,
}

/// The largest icon kept, in pixels per side.
const MAX_SIDE: u32 = 256;

impl FaviconPixels {
    /// A width and height header followed by the pixels.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.pixels.len());
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.pixels);
        bytes
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        let width = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?);
        let height = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?);
        let pixels = bytes.get(8..)?.to_vec();
        let valid = width > 0
            && height > 0
            && width <= MAX_SIDE
            && height <= MAX_SIDE
            && pixels.len() == width as usize * height as usize * 4;
        valid.then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub(crate) fn is_storable(&self) -> bool {
        self.width <= MAX_SIDE && self.height <= MAX_SIDE
    }
}

/// The file holding `url`'s icon in `directory`. Named by a stable hash of
/// the address, so the same page always maps to the same file.
pub(crate) fn path(directory: &Path, url: &str) -> PathBuf {
    directory.join(format!("{:016x}.icon", fnv1a(url.as_bytes())))
}

/// 64-bit FNV-1a, which unlike the standard library's hasher never changes
/// between Rust releases.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
