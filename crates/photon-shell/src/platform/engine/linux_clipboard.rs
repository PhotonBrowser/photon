use gpui::{
    ClipboardEntry, ClipboardHandle, ClipboardItem, ClipboardString, ExternalPaths, Image,
    ImageFormat,
};
use smallvec::SmallVec;
use std::{
    ffi::{CStr, c_char, c_void},
    path::PathBuf,
};
use url::Url;

use super::engine_ffi::embedder::PhotonClipboardEntry;

pub(super) struct SystemClipboard {
    handle: ClipboardHandle,
}

impl SystemClipboard {
    pub(super) fn new(handle: ClipboardHandle) -> Self {
        Self { handle }
    }
}

pub(super) unsafe extern "C" fn read(
    context: *mut c_void,
    add_entry: unsafe extern "C" fn(*mut c_void, *const c_char, *const u8, usize),
    add_entry_context: *mut c_void,
) {
    if context.is_null() {
        return;
    }
    let clipboard = unsafe { &*(context.cast::<SystemClipboard>()) };
    let Some(item) = clipboard.handle.read_from_clipboard() else {
        return;
    };

    let has_text = item
        .entries
        .iter()
        .any(|entry| matches!(entry, ClipboardEntry::String(_)));
    if has_text {
        let text = item
            .entries
            .iter()
            .filter_map(|entry| match entry {
                ClipboardEntry::String(value) => Some(value.text.as_str()),
                _ => None,
            })
            .collect::<String>();
        add_bytes(add_entry, add_entry_context, "text/plain", text.as_bytes());
    }

    let mut seen_images = std::collections::HashSet::new();
    for entry in item.entries {
        match entry {
            ClipboardEntry::Image(image) => {
                let mime_type = image.format.mime_type();
                if seen_images.insert(mime_type) {
                    add_bytes(add_entry, add_entry_context, mime_type, &image.bytes);
                }
            }
            ClipboardEntry::ExternalPaths(paths) => {
                let list = file_uri_list(&paths);
                if !list.is_empty() {
                    add_bytes(
                        add_entry,
                        add_entry_context,
                        "text/uri-list",
                        list.as_bytes(),
                    );
                }
            }
            ClipboardEntry::String(_) => {}
        }
    }
}

pub(super) unsafe extern "C" fn write(
    context: *mut c_void,
    entries: *const PhotonClipboardEntry,
    count: usize,
) {
    if context.is_null() || (entries.is_null() && count != 0) {
        return;
    }
    let clipboard = unsafe { &*(context.cast::<SystemClipboard>()) };
    let mut item = ClipboardItem {
        entries: Vec::new(),
    };
    let mut has_text = false;
    let mut image_formats = std::collections::HashSet::new();
    let mut has_paths = false;
    if count > 0 {
        for entry in unsafe { std::slice::from_raw_parts(entries, count) } {
            if entry.mime_type.is_null() || (entry.data.is_null() && entry.data_size != 0) {
                continue;
            }
            let Ok(mime_type) = unsafe { CStr::from_ptr(entry.mime_type) }.to_str() else {
                continue;
            };
            let bytes = if entry.data_size == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(entry.data, entry.data_size) }
            };
            match mime_type {
                "text/plain" | "text/plain;charset=utf-8" | "UTF8_STRING" => {
                    if !has_text {
                        if let Ok(text) = std::str::from_utf8(bytes) {
                            item.entries
                                .push(ClipboardEntry::String(ClipboardString::new(
                                    text.to_owned(),
                                )));
                            has_text = true;
                        }
                    }
                }
                "text/uri-list" => {
                    if !has_paths {
                        if let Some(paths) = parse_file_uri_list(bytes) {
                            item.entries
                                .push(ClipboardEntry::ExternalPaths(ExternalPaths(paths)));
                            has_paths = true;
                        }
                    }
                }
                _ => {
                    if let Some(format) = ImageFormat::from_mime_type(mime_type) {
                        if image_formats.insert(format) {
                            item.entries.push(ClipboardEntry::Image(Image::from_bytes(
                                format,
                                bytes.to_vec(),
                            )));
                        }
                    }
                }
            }
        }
    }
    clipboard.handle.write_to_clipboard(item);
}

fn add_bytes(
    callback: unsafe extern "C" fn(*mut c_void, *const c_char, *const u8, usize),
    context: *mut c_void,
    mime_type: &str,
    bytes: &[u8],
) {
    let Ok(mime_type) = std::ffi::CString::new(mime_type) else {
        return;
    };
    unsafe { callback(context, mime_type.as_ptr(), bytes.as_ptr(), bytes.len()) };
}

fn file_uri_list(paths: &ExternalPaths) -> String {
    paths
        .0
        .iter()
        .filter_map(|path| Url::from_file_path(path).ok())
        .fold(String::new(), |mut list, url| {
            list.push_str(url.as_str());
            list.push_str("\r\n");
            list
        })
}

fn parse_file_uri_list(bytes: &[u8]) -> Option<SmallVec<[PathBuf; 2]>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let paths = text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| Url::parse(line).ok()?.to_file_path().ok())
        .collect::<SmallVec<[PathBuf; 2]>>();
    (!paths.is_empty()).then_some(paths)
}
