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

    #[repr(C)]
    pub(crate) struct NewWebViewRequest {
        pub popup: bool,
        pub activate: bool,
        pub has_width: bool,
        pub width: i32,
        pub has_height: bool,
        pub height: i32,
        pub has_screen_x: bool,
        pub screen_x: i32,
        pub has_screen_y: bool,
        pub screen_y: i32,
        pub traversable: *mut c_void,
    }

    pub(crate) type NewWebViewCallback = unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const NewWebViewRequest,
        *mut c_char,
        usize,
    );

    #[repr(C)]
    pub(crate) struct ContextMenuItem {
        pub separator: bool,
        pub text: *const c_char,
        pub enabled: bool,
        pub checkable: bool,
        pub checked: bool,
    }

    pub(crate) type ContextMenuCallback =
        unsafe extern "C" fn(*mut c_void, f64, f64, *const ContextMenuItem, usize);

    /// A `<select>` dropdown entry's `kind`.
    pub(crate) const SELECT_ITEM_OPTION: i32 = 0;
    pub(crate) const SELECT_ITEM_GROUP_LABEL: i32 = 1;

    #[repr(C)]
    pub(crate) struct SelectItem {
        pub kind: i32,
        pub label: *const c_char,
        pub id: u32,
        pub selected: bool,
        pub enabled: bool,
    }

    pub(crate) type SelectDropdownCallback =
        unsafe extern "C" fn(*mut c_void, f64, f64, f64, *const SelectItem, usize);
    #[cfg(not(feature = "ui-only"))]
    pub(crate) type EngineProcessCallback = unsafe extern "C" fn(
        *mut c_void,
        u64,
        i32,
        *const c_char,
        *const c_char,
        bool,
        u64,
        bool,
        f64,
        bool,
        u64,
        bool,
        u64,
        bool,
        u64,
        bool,
        u64,
        bool,
        u64,
        bool,
        *const c_char,
        bool,
    );
    pub(crate) type FrameCallback = unsafe extern "C" fn(
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
        u64,
    );

    #[repr(C)]
    pub(crate) struct ViewCallbacks {
        pub callback_data: *mut c_void,
        pub state_callback: Option<
            unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                bool,
                bool,
                bool,
                bool,
                bool,
            ),
        >,
        pub navigation_timing_callback:
            Option<unsafe extern "C" fn(*mut c_void, i32, *const c_char)>,
        pub frame_callback: Option<FrameCallback>,
        pub cursor_callback: Option<unsafe extern "C" fn(*mut c_void, i32)>,
        pub error_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
        pub crash_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
        pub performance_callback:
            Option<unsafe extern "C" fn(*mut c_void, *const PerformanceStats)>,
        pub favicon_callback: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize, i32, i32)>,
        pub audio_state_callback: Option<unsafe extern "C" fn(*mut c_void, bool, bool)>,
        pub dialog_callback: Option<
            unsafe extern "C" fn(*mut c_void, i32, *const c_char, *const c_char, *const c_char),
        >,
        pub navigation_committed_callback: Option<unsafe extern "C" fn(*mut c_void)>,
        pub history_swipe_candidate_callback:
            Option<unsafe extern "C" fn(*mut c_void, u64, bool, f64)>,
        pub crash_recovered_callback: Option<unsafe extern "C" fn(*mut c_void)>,
        pub new_web_view_callback: Option<NewWebViewCallback>,
        pub find_result_callback: Option<unsafe extern "C" fn(*mut c_void, usize, bool, usize)>,
        pub zoom_callback: Option<unsafe extern "C" fn(*mut c_void, f64)>,
        pub page_unresponsive_callback: Option<unsafe extern "C" fn(*mut c_void, bool)>,
        pub context_menu_callback: Option<ContextMenuCallback>,
        pub open_in_new_tab_callback:
            Option<unsafe extern "C" fn(*mut c_void, *const c_char, bool)>,
        pub peek_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
        pub select_dropdown_callback: Option<SelectDropdownCallback>,
        pub file_picker_callback: Option<unsafe extern "C" fn(*mut c_void, bool)>,
        pub permission_callback: Option<
            unsafe extern "C" fn(
                *mut c_void,
                u64,
                *const c_char,
                *const c_char,
                *const c_char,
                *const c_char,
            ),
        >,
        pub permission_cancelled_callback:
            Option<unsafe extern "C" fn(*mut c_void, u64, *const c_char)>,
        #[cfg(target_os = "macos")]
        pub native_metal_presentation: bool,
        #[cfg(target_os = "macos")]
        pub native_backing_callback:
            Option<unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, u32, u32) -> bool>,
        #[cfg(target_os = "macos")]
        pub native_frame_callback:
            Option<unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, i32, i32, f64)>,
    }

    unsafe extern "C" {
        #[cfg(not(feature = "ui-only"))]
        pub fn photon_runtime_create(
            helper_directory: *const c_char,
            profile_path: *const c_char,
            error: *mut c_char,
            capacity: usize,
        ) -> *mut c_void;
        pub fn photon_runtime_clear_browsing_data(
            runtime: *mut c_void,
            since_unix_seconds: i64,
            cache: bool,
            site_data: bool,
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void)>,
        );
        #[cfg(target_os = "macos")]
        pub fn photon_runtime_set_native_release_drain_callback(
            runtime: *mut c_void,
            context: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void)>,
        );
        #[cfg(target_os = "macos")]
        pub fn photon_runtime_schedule_native_release_drain(runtime: *mut c_void);
        pub fn photon_runtime_destroy(runtime: *mut c_void);
        #[cfg(all(
            not(feature = "ui-only"),
            any(target_os = "linux", target_os = "windows")
        ))]
        pub fn photon_runtime_pump(runtime: *mut c_void);
        #[cfg(target_os = "macos")]
        #[cfg(not(feature = "ui-only"))]
        pub fn photon_runtime_use_system_clipboard(runtime: *mut c_void);
        pub fn photon_view_create(
            runtime: *mut c_void,
            width: i32,
            height: i32,
            dpr: f64,
            callbacks: *const ViewCallbacks,
        ) -> *mut c_void;
        pub fn photon_view_create_for_traversable(
            runtime: *mut c_void,
            parent_view: *mut c_void,
            traversable: *mut c_void,
            width: i32,
            height: i32,
            dpr: f64,
            callbacks: *const ViewCallbacks,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
        pub fn photon_view_set_performance_monitor_enabled(view: *mut c_void, enabled: bool);
        pub fn photon_view_set_residency_monitor_enabled(view: *mut c_void, enabled: bool);
        #[cfg(unix)]
        pub fn photon_view_set_task_queue_diagnostics_enabled(
            view: *mut c_void,
            enabled: bool,
        ) -> bool;
        #[cfg(unix)]
        pub fn photon_view_copy_task_queue_diagnostics(
            view: *mut c_void,
            output: *mut c_char,
            capacity: usize,
        ) -> usize;
        pub fn photon_view_release_hidden_memory(view: *mut c_void);
        pub fn photon_view_needs_beforeunload_check(view: *mut c_void) -> bool;
        pub fn photon_view_set_visible(view: *mut c_void, visible: bool);
        pub fn photon_view_set_display_metadata(
            view: *mut c_void,
            display_id: u64,
            refresh_rate: f64,
        );
        pub fn photon_view_set_focus(view: *mut c_void, focused: bool);
        pub fn photon_view_activate_context_menu_item(view: *mut c_void, index: usize);
        pub fn photon_view_select_dropdown_closed(view: *mut c_void, chosen: bool, id: u32);
        pub fn photon_view_file_picker_closed(
            view: *mut c_void,
            paths: *const *const c_char,
            count: usize,
        );
        pub fn photon_view_permission_decided(
            view: *mut c_void,
            request_id: u64,
            document_id: *const c_char,
            granted: bool,
        );
        pub fn photon_view_notify_state(view: *mut c_void);
        pub fn photon_view_copy_window_handle(
            view: *mut c_void,
            handle: *mut c_char,
            capacity: usize,
        );
        pub fn photon_view_set_preferred_color_scheme(view: *mut c_void, color_scheme: i32);
        pub fn photon_view_find_in_page(
            view: *mut c_void,
            query: *const c_char,
            case_sensitive: bool,
            highlight_all_matches: bool,
        );
        pub fn photon_view_find_in_page_step(view: *mut c_void, forward: bool);
        pub fn photon_view_find_in_page_end(view: *mut c_void);
        pub fn photon_view_zoom(view: *mut c_void, step: i32);
        pub fn photon_view_restart_unresponsive_page(view: *mut c_void);
        pub fn photon_view_close_dialog(
            view: *mut c_void,
            dialog_type: i32,
            accepted: bool,
            text: *const c_char,
        );
        #[cfg(all(not(feature = "ui-only"), target_os = "macos"))]
        pub fn photon_runtime_set_system_reduced_motion_preference(
            runtime: *mut c_void,
            reduce_motion: bool,
        );
        pub fn photon_runtime_set_memory_pressure(runtime: *mut c_void, pressured: bool);
        pub fn photon_runtime_set_elastic_scroll_enabled(runtime: *mut c_void, enabled: bool);
        #[cfg(not(feature = "ui-only"))]
        pub fn photon_runtime_set_service_callback(
            runtime: *mut c_void,
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, i32, bool)>,
        );
        #[cfg(unix)]
        pub fn photon_runtime_list_registered_processes(
            runtime: *mut c_void,
            callback_data: *mut c_void,
            callback: Option<
                unsafe extern "C" fn(
                    *mut c_void,
                    i32,
                    *const c_char,
                    bool,
                    u64,
                    bool,
                    i32,
                    bool,
                    u64,
                ),
            >,
        );
        #[cfg(not(feature = "ui-only"))]
        pub fn photon_runtime_list_process_snapshots(
            runtime: *mut c_void,
            callback_data: *mut c_void,
            callback: Option<EngineProcessCallback>,
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
            history_swipe_back_enabled: bool,
            history_swipe_forward_enabled: bool,
            history_swipe_gesture_id: u64,
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
        #[cfg(target_os = "macos")]
        pub fn photon_view_release_native_frame(
            view: *mut c_void,
            backing: u64,
            generation: u64,
            frame: u64,
            consumer_gpu_work_complete: bool,
        );
        #[cfg(target_os = "macos")]
        pub fn photon_view_set_native_metal_presentation(view: *mut c_void, enabled: bool) -> bool;
        #[cfg(target_os = "macos")]
        pub fn photon_reduced_motion_observer_create(
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, bool)>,
        ) -> *mut c_void;
        #[cfg(target_os = "macos")]
        pub fn photon_reduced_motion_observer_destroy(observer: *mut c_void);
        #[cfg(target_os = "macos")]
        pub fn photon_window_observer_create(
            ns_view: *mut c_void,
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, bool, bool)>,
        ) -> *mut c_void;
        #[cfg(target_os = "macos")]
        pub fn photon_window_observer_destroy(observer: *mut c_void);
        #[cfg(target_os = "macos")]
        pub fn photon_default_browser_available() -> bool;
        #[cfg(target_os = "macos")]
        pub fn photon_default_browser_is_current() -> bool;
        #[cfg(target_os = "macos")]
        pub fn photon_default_browser_request();
        pub fn photon_view_navigate(view: *mut c_void, url: *const c_char);
        #[cfg(unix)]
        pub fn photon_automation_view_has_completed_document_load(view: *mut c_void) -> bool;
        #[cfg(unix)]
        pub fn photon_automation_view_document_generation(view: *mut c_void) -> u64;
        #[cfg(unix)]
        pub fn photon_automation_view_install_speedometer_result_observer(
            view: *mut c_void,
            expected_document_generation: u64,
        ) -> bool;
        #[cfg(unix)]
        pub fn photon_automation_view_install_jetstream_result_observer(
            view: *mut c_void,
            expected_document_generation: u64,
        ) -> bool;
        #[cfg(unix)]
        pub fn photon_automation_view_request_jetstream_start(
            view: *mut c_void,
            expected_document_generation: u64,
        ) -> bool;
        pub fn photon_view_reload(view: *mut c_void);
        pub fn photon_view_stop_loading(view: *mut c_void);
        pub fn photon_view_go_back(view: *mut c_void);
        pub fn photon_view_go_forward(view: *mut c_void);
        pub fn photon_view_toggle_audio_mute(view: *mut c_void) -> bool;
        pub fn photon_view_shutdown(view: *mut c_void);
        pub fn photon_view_destroy(view: *mut c_void);
    }
}
