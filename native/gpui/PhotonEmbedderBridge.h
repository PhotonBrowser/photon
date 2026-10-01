#pragma once

#include <cstddef>
#include <cstdint>

extern "C" {

using PhotonFrameCallback = void (*)(void *, int, int, size_t, double,
                                     std::uint8_t const *, size_t, uint64_t,
                                     uint64_t, uint64_t, uint64_t);
using PhotonStateCallback = void (*)(void *, char const *, char const *, bool,
                                     bool, bool);
using PhotonErrorCallback = void (*)(void *, char const *);
using PhotonCursorCallback = void (*)(void *, int);
#if defined(__APPLE__)
using PhotonNativeReleaseDrainCallback = void (*)(void*);
using PhotonNativeBackingCallback = bool (*)(void *, uint64_t, uint64_t, uint32_t,
                                             uint32_t, uint32_t, uint32_t);
using PhotonNativeFrameCallback = void (*)(void *, uint64_t, uint64_t, uint64_t,
                                           uint64_t, int, int, double);
#endif

void *photon_runtime_create(char const *helper_directory, char *error,
                            size_t error_capacity);
void photon_runtime_pump(void *runtime);
#if defined(__APPLE__)
void photon_runtime_set_native_release_drain_callback(
    void *runtime, void *callback_data,
    PhotonNativeReleaseDrainCallback callback);
void photon_runtime_schedule_native_release_drain(void *runtime);
#endif
void photon_runtime_destroy(void *runtime);
void *photon_view_create(void *runtime, int width, int height, double dpr,
                         void *callback_data,
                         PhotonStateCallback state_callback,
                         PhotonFrameCallback frame_callback,
                         PhotonCursorCallback cursor_callback,
                         PhotonErrorCallback error_callback
#if defined(__APPLE__)
                         , bool native_metal_presentation,
                         PhotonNativeBackingCallback native_backing_callback,
                         PhotonNativeFrameCallback native_frame_callback
#endif
                         );
void photon_view_resize(void *view, int width, int height, double dpr);
#if defined(__APPLE__)
void photon_view_release_native_frame(void *view, uint64_t backing_id,
                                      uint64_t generation, uint64_t frame_id);
bool photon_view_set_native_metal_presentation(void *view, bool enabled);
#endif
void photon_view_navigate(void *view, char const *url);
void photon_view_set_focus(void *view, bool focused);
void photon_view_pointer(void *view, int kind, double x, double y, int button,
                         uint8_t buttons, bool shift, bool control, bool alt,
                         bool meta, double wheel_x, double wheel_y,
                         bool precise, int phase, int clicks);
void photon_view_key(void *view, uint16_t key, bool pressed,
                     uint32_t code_point, bool shift, bool control, bool alt,
                     bool meta, bool repeat, bool insert_text);
void photon_view_shutdown(void *view);
void photon_view_destroy(void *view);
}
