#include "PhotonEmbedderBridge.h"

#include <LibPhotonEmbedder/PhotonEmbedder.h>

#include <array>
#include <cstdio>
#include <memory>
#include <string>

namespace {

struct RuntimeHandle {
    std::unique_ptr<Photon::Runtime> runtime;
};

struct ViewHandle {
    std::unique_ptr<Photon::View> view;
};

void copy_error(char* destination, size_t capacity,
    std::string const& message)
{
    if (destination && capacity > 0)
        std::snprintf(destination, capacity, "%s", message.c_str());
}

Photon::ViewCallbacks make_view_callbacks(
    PhotonViewCallbacks const& callbacks,
    void* runtime,
    void* parent_view)
{
    auto callback_data = callbacks.callback_data;
    auto state_callback = callbacks.state_callback;
    auto frame_callback = callbacks.frame_callback;
    auto cursor_callback = callbacks.cursor_callback;
    auto error_callback = callbacks.error_callback;
    auto crash_callback = callbacks.crash_callback;
    auto performance_callback = callbacks.performance_callback;
    auto favicon_callback = callbacks.favicon_callback;
    auto dialog_callback = callbacks.dialog_callback;
    auto navigation_committed_callback = callbacks.navigation_committed_callback;
    auto crash_recovered_callback = callbacks.crash_recovered_callback;
    auto new_web_view_callback = callbacks.new_web_view_callback;

    Photon::ViewCallbacks result;
    result.state_changed = [=](Photon::ViewState const& state) {
        if (state_callback)
            state_callback(callback_data, state.url.c_str(), state.title.c_str(),
                state.loading, state.can_go_back, state.can_go_forward);
    };
    result.frame_ready = [=](std::shared_ptr<Photon::PresentedFrame const> frame) {
        if (frame_callback && frame)
            frame_callback(callback_data, frame->width, frame->height,
                frame->stride, frame->device_pixel_ratio,
                frame->pixels.data(), frame->pixels.size(),
                frame->engine_paint_interval_microseconds,
                frame->bitmap_acquisition_microseconds,
                frame->copy_time_microseconds,
                frame->paint_to_callback_microseconds);
    };
    result.performance_stats_changed = [=](Photon::PerformanceStats const& stats) {
        if (!performance_callback)
            return;
        PhotonPerformanceStats snapshot {
            stats.has_cpu_percent,
            stats.cpu_percent,
            stats.has_memory_bytes,
            stats.memory_bytes,
            stats.has_managed_heap_bytes,
            stats.managed_heap_bytes,
            stats.download_bytes_per_second,
            stats.upload_bytes_per_second,
            stats.has_frames_per_second,
            stats.frames_per_second,
        };
        performance_callback(callback_data, &snapshot);
    };
#if defined(__APPLE__)
    result.native_metal_presentation = callbacks.native_metal_presentation
        && callbacks.native_backing_callback && callbacks.native_frame_callback;
    if (result.native_metal_presentation) {
        auto native_backing_callback = callbacks.native_backing_callback;
        auto native_frame_callback = callbacks.native_frame_callback;
        result.native_backing_registered = [=](Photon::NativeGpuBacking const& backing) {
            return native_backing_callback(callback_data, backing.backing_id,
                backing.generation, backing.width, backing.height,
                backing.pixel_format, backing.iosurface_mach_port);
        };
        result.native_frame_ready = [=](Photon::NativeGpuFrame const& frame) {
            native_frame_callback(callback_data, frame.backing_id, frame.generation,
                frame.frame_id, frame.signal_value, frame.width, frame.height,
                frame.device_pixel_ratio);
        };
    }
#endif
    result.cursor_changed = [=](Photon::Cursor cursor) {
        if (cursor_callback)
            cursor_callback(callback_data, static_cast<int>(cursor));
    };
    result.favicon_changed = [=](Photon::Favicon const* favicon) {
        if (!favicon_callback)
            return;
        if (!favicon) {
            favicon_callback(callback_data, nullptr, 0, 0, 0);
            return;
        }
        favicon_callback(callback_data, favicon->pixels.data(),
            favicon->pixels.size(), favicon->width, favicon->height);
    };
    result.dialog_requested = [=](Photon::DialogRequest const& request) {
        if (dialog_callback)
            dialog_callback(callback_data, static_cast<int>(request.type),
                request.title.c_str(), request.message.c_str(),
                request.default_text.c_str());
    };
    result.navigation_committed = [=] {
        if (navigation_committed_callback)
            navigation_committed_callback(callback_data);
    };
    result.new_web_view_requested = [=](Photon::NewWebViewRequest const& request) {
        if (!new_web_view_callback)
            return std::string {};
        PhotonNewWebViewRequest bridge_request {
            request.popup,
            request.activate,
            request.has_width,
            request.width,
            request.has_height,
            request.height,
            request.has_screen_x,
            request.screen_x,
            request.has_screen_y,
            request.screen_y,
            request.traversable,
        };
        std::array<char, 256> window_handle {};
        new_web_view_callback(callback_data, runtime, parent_view,
            &bridge_request, window_handle.data(), window_handle.size());
        return std::string(window_handle.data());
    };
    result.failed = [=](std::string const& message) {
        if (error_callback)
            error_callback(callback_data, message.c_str());
    };
    result.crashed = [=](std::string const& url) {
        if (crash_callback)
            crash_callback(callback_data, url.c_str());
    };
    result.crash_recovered = [=] {
        if (crash_recovered_callback)
            crash_recovered_callback(callback_data);
    };
    return result;
}

} // namespace

extern "C" void* photon_runtime_create(char const* helper_directory,
    char* error, size_t error_capacity)
{
    std::string message;
    auto runtime = Photon::Runtime::create(
        helper_directory ? helper_directory : "", message);
    if (!runtime) {
        copy_error(error, error_capacity, message);
        return nullptr;
    }
    return new RuntimeHandle { std::move(runtime) };
}

#if defined(__APPLE__)
extern "C" void photon_runtime_set_native_release_drain_callback(
    void* runtime, void* callback_data,
    PhotonNativeReleaseDrainCallback callback)
{
    if (runtime)
        static_cast<RuntimeHandle*>(runtime)->runtime->set_native_release_drain_callback(
            callback_data, callback);
}

extern "C" void photon_runtime_schedule_native_release_drain(void* runtime)
{
    if (runtime)
        static_cast<RuntimeHandle*>(runtime)->runtime->schedule_native_release_drain();
}
#endif

extern "C" void photon_runtime_destroy(void* runtime)
{
    delete static_cast<RuntimeHandle*>(runtime);
}

extern "C" void* photon_view_create(void* runtime, int width, int height,
    double dpr, PhotonViewCallbacks const* callbacks)
{
    if (!runtime || !callbacks)
        return nullptr;
    auto handle = std::make_unique<ViewHandle>();
    auto view = static_cast<RuntimeHandle*>(runtime)->runtime->create_view(
        width, height, dpr, make_view_callbacks(*callbacks, runtime, handle.get()));
    if (!view)
        return nullptr;
    handle->view = std::move(view);
    return handle.release();
}

extern "C" void* photon_view_create_for_traversable(void* runtime,
    void* parent_view, void* traversable, int width, int height, double dpr,
    PhotonViewCallbacks const* callbacks)
{
    if (!runtime || !parent_view || !traversable || !callbacks)
        return nullptr;
    auto& parent = *static_cast<ViewHandle*>(parent_view)->view;
    auto handle = std::make_unique<ViewHandle>();
    auto view = static_cast<RuntimeHandle*>(runtime)->runtime->create_view_for_traversable(
        parent, traversable, width, height, dpr,
        make_view_callbacks(*callbacks, runtime, handle.get()));
    if (!view)
        return nullptr;
    handle->view = std::move(view);
    return handle.release();
}

extern "C" void photon_view_resize(void* view, int width, int height,
    double dpr)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->resize(width, height, dpr);
}

extern "C" void photon_view_set_performance_monitor_enabled(void* view,
    bool enabled)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->set_performance_monitor_enabled(enabled);
}

extern "C" void photon_view_set_visible(void* view, bool visible)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->set_visible(visible);
}

extern "C" void photon_view_set_display_metadata(void* view,
    uint64_t display_id,
    double refresh_rate)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->set_display_metadata(display_id,
            refresh_rate);
}

#if defined(__APPLE__)
extern "C" void photon_view_release_native_frame(void* view,
    uint64_t backing_id,
    uint64_t generation,
    uint64_t frame_id)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->release_native_frame(
            backing_id, generation, frame_id);
}

extern "C" bool photon_view_set_native_metal_presentation(void* view,
    bool enabled)
{
    if (!view)
        return false;
    static_cast<ViewHandle*>(view)->view->set_native_metal_presentation(enabled);
    return true;
}
#endif

extern "C" void photon_view_navigate(void* view, char const* url)
{
    if (view && url)
        static_cast<ViewHandle*>(view)->view->navigate(url);
}

extern "C" void photon_view_reload(void* view)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->reload();
}

extern "C" void photon_view_stop_loading(void* view)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->stop_loading();
}

extern "C" void photon_view_go_back(void* view)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->go_back();
}

extern "C" void photon_view_go_forward(void* view)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->go_forward();
}

extern "C" void photon_view_set_focus(void* view, bool focused)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->set_focus(focused);
}

extern "C" void photon_view_notify_state(void* view)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->notify_state();
}

extern "C" void photon_view_copy_window_handle(void* view, char* handle,
    size_t capacity)
{
    if (!view)
        return;
    copy_error(handle, capacity,
        static_cast<ViewHandle*>(view)->view->window_handle());
}

extern "C" void photon_view_close_dialog(void* view, int type, bool accepted,
    char const* text)
{
    if (!view)
        return;
    auto& engine_view = *static_cast<ViewHandle*>(view)->view;
    switch (static_cast<Photon::DialogType>(type)) {
    case Photon::DialogType::Alert:
        engine_view.alert_closed();
        break;
    case Photon::DialogType::Confirm:
        engine_view.confirm_closed(accepted);
        break;
    case Photon::DialogType::Prompt:
        engine_view.prompt_closed(text ? std::optional<std::string>(text) : std::nullopt);
        break;
    }
}

extern "C" void photon_view_set_preferred_color_scheme(void* view,
    int color_scheme)
{
    if (!view)
        return;
    auto scheme = Photon::PreferredColorScheme::Auto;
    if (color_scheme == 1)
        scheme = Photon::PreferredColorScheme::Dark;
    else if (color_scheme == 2)
        scheme = Photon::PreferredColorScheme::Light;
    static_cast<ViewHandle*>(view)->view->set_preferred_color_scheme(scheme);
}

extern "C" void
photon_runtime_set_system_reduced_motion_preference(void* runtime,
    bool reduce_motion)
{
    if (runtime)
        static_cast<RuntimeHandle*>(runtime)
            ->runtime->set_system_reduced_motion_preference(reduce_motion);
}

extern "C" void photon_runtime_set_service_callback(void* runtime,
    void* callback_data, PhotonServiceCallback callback)
{
    if (!runtime || !callback)
        return;
    static_cast<RuntimeHandle*>(runtime)->runtime->set_service_callback(
        [=](Photon::EngineService service, bool restarted) {
            callback(callback_data, static_cast<int>(service), restarted);
        });
}

extern "C" void photon_view_pointer(void* view, int kind, double x, double y,
    int button, uint8_t buttons, bool shift,
    bool control, bool alt, bool meta,
    double wheel_x, double wheel_y,
    bool precise, int phase, int clicks)
{
    if (!view)
        return;
    Photon::PointerEvent event;
    event.type = static_cast<Photon::PointerType>(kind);
    event.x = x;
    event.y = y;
    event.button = static_cast<Photon::PointerButton>(button);
    event.buttons = buttons;
    event.shift = shift;
    event.control = control;
    event.alt = alt;
    event.meta = meta;
    event.wheel_x = wheel_x;
    event.wheel_y = wheel_y;
    event.precise_wheel = precise;
    event.scroll_phase = static_cast<Photon::ScrollPhase>(phase);
    event.click_count = clicks;
    static_cast<ViewHandle*>(view)->view->send_pointer_event(event);
}

extern "C" void photon_view_key(void* view, uint16_t key, bool pressed,
    uint32_t code_point, bool shift, bool control,
    bool alt, bool meta, bool repeat,
    bool insert_text)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->send_key_event(
            static_cast<Photon::Key>(key), pressed, code_point, shift, control, alt,
            meta, repeat, insert_text);
}

extern "C" void photon_view_shutdown(void* view)
{
    if (view)
        static_cast<ViewHandle*>(view)->view->shutdown();
}

extern "C" void photon_view_destroy(void* view)
{
    delete static_cast<ViewHandle*>(view);
}
