//! Rust declarations for the narrow native embedder C ABI.

pub(super) mod embedder {
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    pub(crate) struct PerformanceStats {
        pub has_cpu_percent: bool,
        pub cpu_percent: f64,
        pub has_memory_bytes: bool,
        pub memory_bytes: u64,
        pub has_managed_heap_bytes: bool,
        pub managed_heap_bytes: u64,
        pub download_bytes_per_second: u64,
        pub upload_bytes_per_second: u64,
        pub has_frames_per_second: bool,
        pub frames_per_second: f64,
    }

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
            crash_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
            performance_callback: Option<
                unsafe extern "C" fn(*mut c_void, *const PerformanceStats),
            >,
            favicon_callback: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize, i32, i32)>,
            dialog_callback: Option<
                unsafe extern "C" fn(*mut c_void, i32, *const c_char, *const c_char, *const c_char),
            >,
            navigation_committed_callback: Option<unsafe extern "C" fn(*mut c_void)>,
            native_metal: bool,
            backing_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, u32, u32) -> bool,
            >,
            native_frame_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, i32, i32, f64),
            >,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
        pub fn photon_view_set_performance_monitor_enabled(view: *mut c_void, enabled: bool);
        pub fn photon_view_set_visible(view: *mut c_void, visible: bool);
        pub fn photon_view_set_display_metadata(
            view: *mut c_void,
            display_id: u64,
            refresh_rate: f64,
        );
        pub fn photon_view_set_focus(view: *mut c_void, focused: bool);
        pub fn photon_view_set_preferred_color_scheme(view: *mut c_void, color_scheme: i32);
        pub fn photon_view_close_dialog(
            view: *mut c_void,
            dialog_type: i32,
            accepted: bool,
            text: *const c_char,
        );
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
        pub fn photon_window_observer_create(
            ns_view: *mut c_void,
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, bool, bool)>,
        ) -> *mut c_void;
        pub fn photon_window_observer_destroy(observer: *mut c_void);
        pub fn photon_view_navigate(view: *mut c_void, url: *const c_char);
        pub fn photon_view_reload(view: *mut c_void);
        pub fn photon_view_stop_loading(view: *mut c_void);
        pub fn photon_view_go_back(view: *mut c_void);
        pub fn photon_view_go_forward(view: *mut c_void);
        pub fn photon_view_shutdown(view: *mut c_void);
        pub fn photon_view_destroy(view: *mut c_void);
    }
}
