#pragma once

#include <cstddef>
#include <cstdint>

extern "C" {

using PhotonFrameCallback = void (*)(void*, int, int, size_t, double,
    std::uint8_t const*, size_t, uint64_t,
    uint64_t, uint64_t, uint64_t);
using PhotonStateCallback = void (*)(void*, char const*, char const*, bool,
    bool, bool);
using PhotonErrorCallback = void (*)(void*, char const*);
using PhotonCrashCallback = void (*)(void*, char const*);
using PhotonCursorCallback = void (*)(void*, int);
struct PhotonPerformanceStats {
    bool has_cpu_percent;
    double cpu_percent;
    bool has_memory_bytes;
    uint64_t memory_bytes;
    bool has_managed_heap_bytes;
    uint64_t managed_heap_bytes;
    uint64_t download_bytes_per_second;
    uint64_t upload_bytes_per_second;
    bool has_frames_per_second;
    double frames_per_second;
};
using PhotonPerformanceCallback = void (*)(void*, PhotonPerformanceStats const*);
// Called with straight-alpha BGRA8888 pixels and their width and height, or
// with null pixels when the page has no icon.
using PhotonFaviconCallback = void (*)(void*, std::uint8_t const*, size_t, int,
    int);
#if defined(__APPLE__)
using PhotonReducedMotionChangedCallback = void (*)(void*, bool);
// Called with whether the window is visible and whether its display or the
// display's parameters (such as refresh rate) may have changed.
using PhotonWindowChangedCallback = void (*)(void*, bool, bool);
using PhotonNativeReleaseDrainCallback = void (*)(void*);
using PhotonNativeBackingCallback = bool (*)(void*, uint64_t, uint64_t, uint32_t,
    uint32_t, uint32_t, uint32_t);
using PhotonNativeFrameCallback = void (*)(void*, uint64_t, uint64_t, uint64_t,
    uint64_t, int, int, double);
#endif

void* photon_runtime_create(char const* helper_directory, char* error,
    size_t error_capacity);
#if defined(__APPLE__)
void photon_runtime_set_native_release_drain_callback(
    void* runtime, void* callback_data,
    PhotonNativeReleaseDrainCallback callback);
void photon_runtime_schedule_native_release_drain(void* runtime);
#endif
void photon_runtime_destroy(void* runtime);
void* photon_view_create(void* runtime, int width, int height, double dpr,
    void* callback_data,
    PhotonStateCallback state_callback,
    PhotonFrameCallback frame_callback,
    PhotonCursorCallback cursor_callback,
    PhotonErrorCallback error_callback,
    PhotonCrashCallback crash_callback,
    PhotonPerformanceCallback performance_callback,
    PhotonFaviconCallback favicon_callback
#if defined(__APPLE__)
    ,
    bool native_metal_presentation,
    PhotonNativeBackingCallback native_backing_callback,
    PhotonNativeFrameCallback native_frame_callback
#endif
);
void photon_view_resize(void* view, int width, int height, double dpr);
void photon_view_set_performance_monitor_enabled(void* view, bool enabled);
void photon_view_set_visible(void* view, bool visible);
void photon_view_set_display_metadata(void* view, uint64_t display_id,
    double refresh_rate);
#if defined(__APPLE__)
void photon_view_release_native_frame(void* view, uint64_t backing_id,
    uint64_t generation, uint64_t frame_id);
bool photon_view_set_native_metal_presentation(void* view, bool enabled);
#endif
void photon_view_navigate(void* view, char const* url);
void photon_view_reload(void* view);
void photon_view_stop_loading(void* view);
void photon_view_go_back(void* view);
void photon_view_go_forward(void* view);
void photon_view_set_focus(void* view, bool focused);
// 0 follows the engine default, 1 prefers dark, 2 prefers light.
void photon_view_set_preferred_color_scheme(void* view, int color_scheme);
void photon_runtime_set_system_reduced_motion_preference(void* runtime,
    bool reduce_motion);
void photon_view_pointer(void* view, int kind, double x, double y, int button,
    uint8_t buttons, bool shift, bool control, bool alt,
    bool meta, double wheel_x, double wheel_y,
    bool precise, int phase, int clicks);
void photon_view_key(void* view, uint16_t key, bool pressed,
    uint32_t code_point, bool shift, bool control, bool alt,
    bool meta, bool repeat, bool insert_text);
void photon_view_shutdown(void* view);
void photon_view_destroy(void* view);
#if defined(__APPLE__)
void* photon_reduced_motion_observer_create(
    void* callback_data, PhotonReducedMotionChangedCallback callback);
void photon_reduced_motion_observer_destroy(void* observer);
void* photon_window_observer_create(void* ns_view, void* callback_data,
    PhotonWindowChangedCallback callback);
void photon_window_observer_destroy(void* observer);
#endif
}
