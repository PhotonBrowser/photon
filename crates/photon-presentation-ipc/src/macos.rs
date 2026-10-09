//! Safe Rust client and service entry point for the presentation XPC transport.
#![allow(unexpected_cfgs)]

use anyhow::{Context, Result, ensure};
use foreign_types::ForeignType;
use mach2::port::{MACH_PORT_NULL, mach_port_t};
use metal::{DeviceRef, SharedEvent};
use objc::{msg_send, sel, sel_impl};
use std::{
    ffi::{CString, c_char, c_void},
    ptr::NonNull,
};

unsafe extern "C" {
    fn photon_presentation_xpc_run_service(service_name: *const c_char) -> i32;
    fn photon_presentation_xpc_connect(service_name: *const c_char) -> *mut c_void;
    fn photon_presentation_xpc_disconnect(connection: *mut c_void);
    fn photon_presentation_xpc_copy_event_handle(
        connection: *mut c_void,
        channel: *const c_char,
        registry_id: *mut u64,
    ) -> *mut c_void;
    fn photon_presentation_xpc_register_backing(
        connection: *mut c_void,
        channel: *const c_char,
        backing: u64,
        generation: u64,
        width: u32,
        height: u32,
        pixel_format: u32,
        port: mach_port_t,
    ) -> bool;
    fn photon_presentation_xpc_copy_backing(
        connection: *mut c_void,
        channel: *const c_char,
        backing: u64,
        generation: u64,
        port: *mut mach_port_t,
        width: *mut u32,
        height: *mut u32,
        pixel_format: *mut u32,
    ) -> bool;
    fn photon_presentation_xpc_unregister_backing(
        connection: *mut c_void,
        channel: *const c_char,
        backing: u64,
        generation: u64,
    ) -> bool;
    fn photon_presentation_xpc_release_object(object: *mut c_void);
}

pub struct Channel(NonNull<c_void>);
unsafe impl Send for Channel {}
unsafe impl Sync for Channel {}

impl Channel {
    pub fn connect(service: &str) -> Result<Self> {
        let service = CString::new(service).context("XPC service name contains NUL")?;
        NonNull::new(unsafe { photon_presentation_xpc_connect(service.as_ptr()) })
            .map(Self)
            .context("could not connect to Photon presentation broker")
    }

    pub fn import_shared_event(
        &self,
        channel: &str,
        device: &DeviceRef,
    ) -> Result<Option<(SharedEvent, u64)>> {
        let channel = CString::new(channel).context("channel ID contains NUL")?;
        let mut registry_id = 0;
        let handle = NonNull::new(unsafe {
            photon_presentation_xpc_copy_event_handle(
                self.0.as_ptr(),
                channel.as_ptr(),
                &mut registry_id,
            )
        });
        let Some(handle) = handle else {
            return Ok(None);
        };
        let _guard = ObjectGuard(handle);
        ensure!(
            registry_id == device.registry_id(),
            "Metal registry ID mismatch: producer={registry_id}, consumer={}",
            device.registry_id()
        );
        let event: *mut metal::MTLSharedEvent =
            unsafe { msg_send![device, newSharedEventWithHandle: handle.as_ptr()] };
        ensure!(
            !event.is_null(),
            "could not import producer shared event into GPUI device"
        );
        Ok(Some((unsafe { SharedEvent::from_ptr(event) }, registry_id)))
    }

    pub fn register_backing(
        &self,
        channel: &str,
        backing: u64,
        generation: u64,
        width: u32,
        height: u32,
        pixel_format: u32,
        port: mach_port_t,
    ) -> Result<()> {
        let channel = CString::new(channel).context("channel ID contains NUL")?;
        ensure!(
            unsafe {
                photon_presentation_xpc_register_backing(
                    self.0.as_ptr(),
                    channel.as_ptr(),
                    backing,
                    generation,
                    width,
                    height,
                    pixel_format,
                    port,
                )
            },
            "broker rejected IOSurface registration"
        );
        Ok(())
    }

    /// Asks the broker to drop its send right for a backing, without waiting.
    /// Its IOSurface is freed once no process holds it any longer.
    pub fn unregister_backing(&self, channel: &str, backing: u64, generation: u64) -> Result<()> {
        let channel = CString::new(channel).context("channel ID contains NUL")?;
        ensure!(
            unsafe {
                photon_presentation_xpc_unregister_backing(
                    self.0.as_ptr(),
                    channel.as_ptr(),
                    backing,
                    generation,
                )
            },
            "could not send IOSurface unregistration"
        );
        Ok(())
    }

    pub fn import_backing(
        &self,
        channel: &str,
        backing: u64,
        generation: u64,
    ) -> Result<(mach_port_t, u32, u32, u32)> {
        let channel = CString::new(channel).context("channel ID contains NUL")?;
        let (mut port, mut width, mut height, mut format) = (MACH_PORT_NULL, 0, 0, 0);
        ensure!(
            unsafe {
                photon_presentation_xpc_copy_backing(
                    self.0.as_ptr(),
                    channel.as_ptr(),
                    backing,
                    generation,
                    &mut port,
                    &mut width,
                    &mut height,
                    &mut format,
                )
            },
            "broker has no IOSurface for backing {backing}/{generation}"
        );
        Ok((port, width, height, format))
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        unsafe { photon_presentation_xpc_disconnect(self.0.as_ptr()) }
    }
}

struct ObjectGuard(NonNull<c_void>);
impl Drop for ObjectGuard {
    fn drop(&mut self) {
        unsafe { photon_presentation_xpc_release_object(self.0.as_ptr()) }
    }
}

/// Runs the macOS presentation broker service for `service_name`.
pub fn run_service(service_name: &str) -> i32 {
    let Ok(service_name) = CString::new(service_name) else {
        return 2;
    };
    unsafe { photon_presentation_xpc_run_service(service_name.as_ptr()) }
}
