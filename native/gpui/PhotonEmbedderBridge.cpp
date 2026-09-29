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

extern "C" void photon_runtime_pump(void *runtime) {
  if (runtime)
    static_cast<RuntimeHandle *>(runtime)->runtime->pump();
}

extern "C" void photon_runtime_destroy(void *runtime) {
  delete static_cast<RuntimeHandle *>(runtime);
}

extern "C" void *photon_view_create(void *runtime, int width, int height,
                                    double dpr, void *callback_data,
                                    PhotonStateCallback state_callback,
                                    PhotonFrameCallback frame_callback,
                                    PhotonCursorCallback cursor_callback,
                                    PhotonErrorCallback error_callback) {
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
