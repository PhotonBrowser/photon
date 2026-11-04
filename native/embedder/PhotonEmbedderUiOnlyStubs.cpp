#include "PhotonEmbedderBridge.h"

// The UI-only shell does not build or link Photon Engine. Keep its embedder
// calls inert so the browser chrome can be built and previewed on its own.
extern "C" {

void* photon_runtime_create(char const*, char const*, char*, size_t)
{
    return nullptr;
}

void photon_runtime_peer_certificate(void*, char const*, void*, PhotonCertificateCallback) { }
void photon_runtime_allow_certificate_errors_for_host(void*, char const*) { }
void photon_runtime_clear_browsing_data(void*, int64_t, bool, bool, void*,
    PhotonBrowsingDataClearedCallback)
{
}
void photon_runtime_clear_site_data(void*, char const*, void*, PhotonBrowsingDataClearedCallback) { }

#if defined(__linux__)
void photon_runtime_set_system_clipboard_callbacks(void*, void*, PhotonClipboardReadCallback,
    PhotonClipboardWriteCallback)
{
}
#endif

#if defined(__APPLE__)
void photon_runtime_set_native_release_drain_callback(void*, void*,
    PhotonNativeReleaseDrainCallback)
{
}
void photon_runtime_schedule_native_release_drain(void*) { }
#endif

void photon_runtime_destroy(void*) { }
void photon_runtime_pump(void*) { }

#if defined(__APPLE__)
void photon_runtime_use_system_clipboard(void*) { }
#endif

void photon_runtime_set_external_apps(void*, void*, PhotonExternalAppNameCallback,
    PhotonExternalAppOpenCallback)
{
}

void* photon_view_create(void*, int, int, double, PhotonViewCallbacks const*, bool)
{
    return nullptr;
}
void* photon_view_create_for_traversable(void*, void*, void*, int, int, double,
    PhotonViewCallbacks const*)
{
    return nullptr;
}
void photon_view_resize(void*, int, int, double) { }
void photon_view_set_performance_monitor_enabled(void*, bool) { }
void photon_view_set_residency_monitor_enabled(void*, bool) { }
bool photon_view_set_task_queue_diagnostics_enabled(void*, bool) { return false; }
size_t photon_view_copy_task_queue_diagnostics(void*, char* output, size_t capacity)
{
    if (output && capacity > 0)
        output[0] = '\0';
    return 0;
}
void photon_view_release_hidden_memory(void*) { }
bool photon_view_needs_beforeunload_check(void*) { return false; }
void photon_view_set_visible(void*, bool) { }
void photon_view_set_display_metadata(void*, uint64_t, double) { }

#if defined(__APPLE__)
void photon_view_release_native_frame(void*, uint64_t, uint64_t, uint64_t, bool) { }
bool photon_view_set_native_metal_presentation(void*, bool) { return false; }
#endif

void photon_view_navigate(void*, char const*) { }
bool photon_automation_view_has_completed_document_load(void*) { return false; }
uint64_t photon_automation_view_document_generation(void*) { return 0; }
bool photon_automation_view_install_speedometer_result_observer(void*, uint64_t) { return false; }
bool photon_automation_view_install_jetstream_result_observer(void*, uint64_t) { return false; }
bool photon_automation_view_request_jetstream_start(void*, uint64_t) { return false; }
void photon_view_reload(void*) { }
void photon_view_stop_loading(void*) { }
void photon_view_go_back(void*) { }
void photon_view_go_forward(void*) { }
bool photon_view_toggle_audio_mute(void*) { return false; }
void photon_view_set_microphone_permission(void*, char const*, bool) { }
void photon_view_set_focus(void*, bool) { }
void photon_view_activate_context_menu_item(void*, size_t) { }
void photon_view_select_dropdown_closed(void*, bool, uint32_t) { }
void photon_view_file_picker_closed(void*, char const* const*, size_t) { }
void photon_view_external_url_answered(void*, uint64_t, bool) { }
void photon_view_download_path_chosen(void*, uint64_t, char const*) { }
void photon_view_permission_decided(void*, uint64_t, char const*, bool) { }
void photon_view_notify_state(void*) { }
void photon_view_copy_window_handle(void*, char* handle, size_t capacity)
{
    if (handle && capacity > 0)
        handle[0] = '\0';
}
void photon_view_set_preferred_color_scheme(void*, int) { }
void photon_view_set_force_dark_mode_enabled(void*, bool) { }
void photon_view_set_fullscreen(void*, bool) { }
void photon_view_exit_fullscreen(void*) { }
void photon_view_close_dialog(void*, int, bool, char const*) { }
void photon_view_find_in_page(void*, char const*, bool, bool) { }
void photon_view_find_in_page_step(void*, bool) { }
void photon_view_find_in_page_end(void*) { }
void photon_view_zoom(void*, int) { }
void photon_view_pinch(void*, double, double, bool, bool, bool, bool, double) { }
void photon_view_restart_unresponsive_page(void*) { }

#if defined(__APPLE__)
void photon_runtime_set_system_reduced_motion_preference(void*, bool) { }
#endif

void photon_runtime_set_memory_pressure(void*, bool) { }
void photon_runtime_set_elastic_scroll_enabled(void*, bool) { }
void photon_runtime_set_service_callback(void*, void*, PhotonServiceCallback) { }
void photon_runtime_set_download_callbacks(void*, void*, PhotonDownloadChangedCallback,
    PhotonDownloadRemovedCallback)
{
}
void photon_runtime_download_action(void*, uint64_t, int) { }
void photon_runtime_clear_inactive_downloads(void*) { }
void photon_runtime_list_registered_processes(void*, void*, PhotonRegisteredProcessCallback) { }
void photon_runtime_list_process_snapshots(void*, void*, PhotonEngineProcessCallback) { }
void photon_view_pointer(void*, int, double, double, int, uint8_t, bool, bool, bool, bool,
    double, double, bool, int, int, bool, bool, uint64_t)
{
}
void photon_view_key(void*, uint16_t, bool, uint32_t, bool, bool, bool, bool, bool, bool) { }
void photon_view_set_marked_text(void*, char const*) { }
void photon_view_commit_text(void*, char const*, int32_t, int32_t) { }
void photon_view_unmark_text(void*) { }
void photon_view_http_credentials_provided(void*, uint64_t, bool, char const*, char const*) { }
void photon_view_shutdown(void*) { }
void photon_view_destroy(void*) { }
}
