//! Rust declarations for the narrow native embedder C ABI.

pub(super) mod embedder {
    use std::ffi::{c_char, c_void};
    unsafe extern "C" {
        pub fn photon_runtime_create(
            helper_directory: *const c_char,
            error: *mut c_char,
            capacity: usize,
        ) -> *mut c_void;
        pub fn photon_runtime_set_native_release_drain_callback(
            runtime: *mut c_void,
            context: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void)>,
        );
        pub fn photon_runtime_schedule_native_release_drain(runtime: *mut c_void);
        pub fn photon_runtime_destroy(runtime: *mut c_void);
        pub fn photon_view_create(
            runtime: *mut c_void,
            width: i32,
            height: i32,
            dpr: f64,
            context: *mut c_void,
            state_callback: Option<
                unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, bool, bool, bool),
            >,
            frame_callback: Option<
                unsafe extern "C" fn(
                    *mut c_void,
                    i32,
                    i32,
                    usize,
                    f64,
                    *const u8,
                    usize,
                    u64,
                    u64,
                    u64,
                    u64,
                ),
            >,
            cursor_callback: Option<unsafe extern "C" fn(*mut c_void, i32)>,
            error_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
            native_metal: bool,
            backing_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, u32, u32) -> bool,
            >,
            native_frame_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, i32, i32, f64),
            >,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
        pub fn photon_view_set_focus(view: *mut c_void, focused: bool);
        pub fn photon_runtime_set_system_reduced_motion_preference(
            runtime: *mut c_void,
            reduce_motion: bool,
        );
        pub fn photon_view_pointer(
            view: *mut c_void,
            kind: i32,
            x: f64,
            y: f64,
            button: i32,
            buttons: u8,
            shift: bool,
            control: bool,
            alt: bool,
            meta: bool,
            wheel_x: f64,
            wheel_y: f64,
            precise: bool,
            phase: i32,
            clicks: i32,
        );
        pub fn photon_view_key(
            view: *mut c_void,
            key: u16,
            pressed: bool,
            code_point: u32,
            shift: bool,
            control: bool,
            alt: bool,
            meta: bool,
            repeat: bool,
            insert_text: bool,
        );
        pub fn photon_view_release_native_frame(
            view: *mut c_void,
            backing: u64,
            generation: u64,
            frame: u64,
        );
        pub fn photon_view_set_native_metal_presentation(view: *mut c_void, enabled: bool) -> bool;
        pub fn photon_reduced_motion_observer_create(
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, bool)>,
        ) -> *mut c_void;
        pub fn photon_reduced_motion_observer_destroy(observer: *mut c_void);
        pub fn photon_view_navigate(view: *mut c_void, url: *const c_char);
        pub fn photon_view_shutdown(view: *mut c_void);
        pub fn photon_view_destroy(view: *mut c_void);
    }
}
