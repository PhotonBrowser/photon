#include "PhotonEmbedderBridge.h"

#include <LibPhotonEmbedder/PhotonEmbedder.h>

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

void copy_error(char *destination, size_t capacity,
                std::string const &message) {
  if (destination && capacity > 0)
    std::snprintf(destination, capacity, "%s", message.c_str());
}

} // namespace

extern "C" void *photon_runtime_create(char const *helper_directory,
                                       char *error, size_t error_capacity) {
  std::string message;
  auto runtime = Photon::Runtime::create(
      helper_directory ? helper_directory : "", message);
  if (!runtime) {
    copy_error(error, error_capacity, message);
    return nullptr;
  }
  return new RuntimeHandle{std::move(runtime)};
}

#if defined(__APPLE__)
extern "C" void photon_runtime_set_native_release_drain_callback(
    void *runtime, void *callback_data,
    PhotonNativeReleaseDrainCallback callback) {
  if (runtime)
    static_cast<RuntimeHandle *>(runtime)->runtime->set_native_release_drain_callback(
        callback_data, callback);
}

extern "C" void photon_runtime_schedule_native_release_drain(void *runtime) {
  if (runtime)
    static_cast<RuntimeHandle *>(runtime)->runtime->schedule_native_release_drain();
}
#endif

extern "C" void photon_runtime_destroy(void *runtime) {
  delete static_cast<RuntimeHandle *>(runtime);
}

extern "C" void *photon_view_create(void *runtime, int width, int height,
                                    double dpr, void *callback_data,
                                    PhotonStateCallback state_callback,
                                    PhotonFrameCallback frame_callback,
                                    PhotonCursorCallback cursor_callback,
                                    PhotonErrorCallback error_callback
#if defined(__APPLE__)
                                    , bool native_metal_presentation,
                                    PhotonNativeBackingCallback native_backing_callback,
                                    PhotonNativeFrameCallback native_frame_callback
#endif
                                    ) {
  if (!runtime)
    return nullptr;

  Photon::ViewCallbacks callbacks;
  callbacks.state_changed = [=](Photon::ViewState const &state) {
    if (state_callback)
      state_callback(callback_data, state.url.c_str(), state.title.c_str(),
                     state.loading, state.can_go_back, state.can_go_forward);
  };
  callbacks.frame_ready =
      [=](std::shared_ptr<Photon::PresentedFrame const> frame) {
        if (frame_callback && frame)
          frame_callback(callback_data, frame->width, frame->height,
                         frame->stride, frame->device_pixel_ratio,
                         frame->pixels.data(), frame->pixels.size(),
                         frame->engine_paint_interval_microseconds,
                         frame->bitmap_acquisition_microseconds,
                         frame->copy_time_microseconds,
                         frame->paint_to_callback_microseconds);
      };
#if defined(__APPLE__)
  callbacks.native_metal_presentation = native_metal_presentation
      && native_backing_callback && native_frame_callback;
  if (callbacks.native_metal_presentation) {
    callbacks.native_backing_registered = [=](Photon::NativeGpuBacking const &backing) {
      return native_backing_callback(callback_data, backing.backing_id,
                                     backing.generation, backing.width,
                                     backing.height, backing.pixel_format,
                                     backing.iosurface_mach_port);
    };
    callbacks.native_frame_ready = [=](Photon::NativeGpuFrame const &frame) {
      native_frame_callback(callback_data, frame.backing_id, frame.generation,
                            frame.frame_id, frame.signal_value, frame.width, frame.height,
                            frame.device_pixel_ratio);
    };
  }
#endif
  callbacks.cursor_changed = [=](Photon::Cursor cursor) {
    if (cursor_callback)
      cursor_callback(callback_data, static_cast<int>(cursor));
  };
  callbacks.failed = [=](std::string const &message) {
    if (error_callback)
      error_callback(callback_data, message.c_str());
  };

  auto view = static_cast<RuntimeHandle *>(runtime)->runtime->create_view(
      width, height, dpr, std::move(callbacks));
  if (!view)
    return nullptr;
  return new ViewHandle{std::move(view)};
}

extern "C" void photon_view_resize(void *view, int width, int height,
                                   double dpr) {
  if (view)
    static_cast<ViewHandle *>(view)->view->resize(width, height, dpr);
}

#if defined(__APPLE__)
extern "C" void photon_view_release_native_frame(void *view,
                                                  uint64_t backing_id,
                                                  uint64_t generation,
                                                  uint64_t frame_id) {
  if (view)
    static_cast<ViewHandle *>(view)->view->release_native_frame(
        backing_id, generation, frame_id);
}

extern "C" bool photon_view_set_native_metal_presentation(void *view,
                                                            bool enabled) {
  if (!view)
    return false;
  static_cast<ViewHandle *>(view)->view->set_native_metal_presentation(enabled);
  return true;
}
#endif

extern "C" void photon_view_navigate(void *view, char const *url) {
  if (view && url)
    static_cast<ViewHandle *>(view)->view->navigate(url);
}

extern "C" void photon_view_set_focus(void *view, bool focused) {
  if (view)
    static_cast<ViewHandle *>(view)->view->set_focus(focused);
}

extern "C" void photon_view_pointer(void *view, int kind, double x, double y,
                                    int button, uint8_t buttons, bool shift,
                                    bool control, bool alt, bool meta,
                                    double wheel_x, double wheel_y,
                                    bool precise, int phase, int clicks) {
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
  static_cast<ViewHandle *>(view)->view->send_pointer_event(event);
}

extern "C" void photon_view_key(void *view, uint16_t key, bool pressed,
                                uint32_t code_point, bool shift, bool control,
                                bool alt, bool meta, bool repeat,
                                bool insert_text) {
  if (view)
    static_cast<ViewHandle *>(view)->view->send_key_event(
        static_cast<Photon::Key>(key), pressed, code_point, shift, control, alt,
        meta, repeat, insert_text);
}

extern "C" void photon_view_shutdown(void *view) {
  if (view)
    static_cast<ViewHandle *>(view)->view->shutdown();
}

extern "C" void photon_view_destroy(void *view) {
  delete static_cast<ViewHandle *>(view);
}
