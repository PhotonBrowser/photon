#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>

#include "PhotonEmbedderBridge.h"

#include <atomic>
#include <memory>

struct WindowObserverState {
  std::atomic<bool> active{true};
  void *callback_data{nullptr};
  PhotonWindowChangedCallback callback{nullptr};
};

struct WindowObserver {
  NSMutableArray *notification_observers{nil};
  std::shared_ptr<WindowObserverState> state;
};

static bool window_is_visible(NSWindow *window) {
  return (window.occlusionState & NSWindowOcclusionStateVisible) != 0;
}

extern "C" void *photon_window_observer_create(void *ns_view,
                                               void *callback_data,
                                               PhotonWindowChangedCallback callback) {
  if (!ns_view || !callback)
    return nullptr;
  NSWindow *window = ((__bridge NSView *)ns_view).window;
  if (!window)
    return nullptr;

  auto state = std::make_shared<WindowObserverState>();
  state->callback_data = callback_data;
  state->callback = callback;
  auto *observer = new WindowObserver;
  observer->state = state;
  observer->notification_observers = [NSMutableArray array];

  auto *center = [NSNotificationCenter defaultCenter];
  // A weak reference keeps the window's lifetime independent of these blocks.
  __weak NSWindow *weak_window = window;
  auto observe = ^(NSNotificationName name, id object, bool display_changed) {
    id notification_observer = [center
        addObserverForName:name
                    object:object
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *) {
                  NSWindow *observed = weak_window;
                  if (observed && state->active.load(std::memory_order_acquire))
                    state->callback(state->callback_data, window_is_visible(observed),
                                    display_changed);
                }];
    [observer->notification_observers addObject:notification_observer];
  };
  observe(NSWindowDidChangeOcclusionStateNotification, window, false);
  observe(NSWindowDidChangeScreenNotification, window, true);
  // Covers refresh-rate and resolution changes on the window's current display.
  observe(NSApplicationDidChangeScreenParametersNotification, nil, true);

  state->callback(state->callback_data, window_is_visible(window), false);
  return observer;
}

extern "C" void photon_window_observer_destroy(void *opaque_observer) {
  auto *observer = static_cast<WindowObserver *>(opaque_observer);
  if (!observer)
    return;

  observer->state->active.store(false, std::memory_order_release);
  for (id notification_observer in observer->notification_observers)
    [[NSNotificationCenter defaultCenter] removeObserver:notification_observer];
  observer->notification_observers = nil;
  delete observer;
}
