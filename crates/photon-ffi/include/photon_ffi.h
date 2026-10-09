#ifndef PHOTON_FFI_H
#define PHOTON_FFI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PhotonBrowserHandle PhotonBrowserHandle;

typedef enum PhotonBrowserStringField {
    PHOTON_BROWSER_STRING_URL = 0,
    PHOTON_BROWSER_STRING_TITLE = 1,
    PHOTON_BROWSER_STRING_ERROR = 2,
} PhotonBrowserStringField;

typedef enum PhotonBrowserFlag {
    PHOTON_BROWSER_FLAG_LOADING = 0,
    PHOTON_BROWSER_FLAG_CAN_GO_BACK = 1,
    PHOTON_BROWSER_FLAG_CAN_GO_FORWARD = 2,
} PhotonBrowserFlag;

typedef enum PhotonBrowserCommand {
    PHOTON_BROWSER_COMMAND_RELOAD = 1,
    PHOTON_BROWSER_COMMAND_BACK = 2,
    PHOTON_BROWSER_COMMAND_FORWARD = 3,
    PHOTON_BROWSER_COMMAND_STOP_LOADING = 4,
} PhotonBrowserCommand;

/// Creates a browser state handle. Destroy it with photon_browser_destroy.
PhotonBrowserHandle* photon_browser_create(void);

/// Destroys a handle returned by photon_browser_create. Null is accepted.
void photon_browser_destroy(PhotonBrowserHandle* handle);

/// Returns a URL, title, or error string. The pointer is borrowed until the
/// handle is mutated or destroyed. Unknown fields and null handles return null.
char const* photon_browser_string(PhotonBrowserHandle const* handle,
    uint32_t field);

/// Returns a loading or history flag. Unknown fields and null handles return
/// false.
bool photon_browser_flag(PhotonBrowserHandle const* handle, uint32_t field);

/// Applies a complete view state snapshot. Null URL and title pointers are
/// treated as empty strings.
void photon_browser_update(PhotonBrowserHandle* handle, char const* url,
    char const* title, bool loading, bool can_go_back,
    bool can_go_forward);

/// Marks a navigation as started. Reuse the displayed frame when it remains
/// the visible page while the new navigation loads.
void photon_browser_navigation_started(PhotonBrowserHandle* handle,
    bool reuse_displayed_frame);

/// Marks the first frame for the active navigation as presented.
void photon_browser_frame_presented(PhotonBrowserHandle* handle);

/// Cancels the active navigation and restores the last committed address.
void photon_browser_cancel_navigation(PhotonBrowserHandle* handle);

/// Resolves address-or-search input into a null-terminated URL in `output`.
/// Returns false for invalid input or when `capacity` is too small.
bool photon_browser_navigate(PhotonBrowserHandle* handle, char const* input,
    char* output, size_t capacity);

/// Returns the last navigation error as a borrowed string, or null when none
/// is present. The pointer is valid until the handle is mutated or destroyed.
char const* photon_browser_error(PhotonBrowserHandle const* handle);

/// Applies a load failure reported by the native Engine adapter.
void photon_browser_load_failed(PhotonBrowserHandle* handle,
    char const* message);

/// Validates a reload, back, forward, or stop-loading command value.
bool photon_browser_command(PhotonBrowserHandle const* handle,
    uint32_t command);

#ifdef __cplusplus
}
#endif

#endif // PHOTON_FFI_H
