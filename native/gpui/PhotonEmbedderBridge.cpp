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
                         frame->pixels.data(), frame->pixels.size());
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

extern "C" void photon_view_shutdown(void *view) {
  if (view)
    static_cast<ViewHandle *>(view)->view->shutdown();
}

extern "C" void photon_view_destroy(void *view) {
  delete static_cast<ViewHandle *>(view);
}
