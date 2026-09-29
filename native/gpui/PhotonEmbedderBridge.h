#pragma once

#include <cstddef>
#include <cstdint>

extern "C" {

using PhotonFrameCallback = void (*)(void *, int, int, size_t, double,
                                     std::uint8_t const *, size_t);
using PhotonStateCallback = void (*)(void *, char const *, char const *, bool,
                                     bool, bool);
using PhotonErrorCallback = void (*)(void *, char const *);

void *photon_runtime_create(char const *helper_directory, char *error,
                            size_t error_capacity);
void photon_runtime_pump(void *runtime);
void photon_runtime_destroy(void *runtime);
void *photon_view_create(void *runtime, int width, int height, double dpr,
                         void *callback_data,
                         PhotonStateCallback state_callback,
                         PhotonFrameCallback frame_callback,
                         PhotonErrorCallback error_callback);
void photon_view_resize(void *view, int width, int height, double dpr);
void photon_view_navigate(void *view, char const *url);
void photon_view_shutdown(void *view);
void photon_view_destroy(void *view);
}
