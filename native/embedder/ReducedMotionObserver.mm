#import <AppKit/AppKit.h>
#import <CoreFoundation/CoreFoundation.h>

#include "PhotonEmbedderBridge.h"

#include <atomic>
#include <memory>

struct ReducedMotionObserverState {
  std::atomic<bool> active{true};
  void *callback_data{nullptr};
  PhotonReducedMotionChangedCallback callback{nullptr};
};

struct ReducedMotionObserver {
  void *notification_observer{nullptr};
  std::shared_ptr<ReducedMotionObserverState> state;
};

extern "C" void *photon_reduced_motion_observer_create(
    void *callback_data, PhotonReducedMotionChangedCallback callback) {
  if (!callback)
    return nullptr;

  auto *workspace = [NSWorkspace sharedWorkspace];
  auto state = std::make_shared<ReducedMotionObserverState>();
  state->callback_data = callback_data;
  state->callback = callback;
  auto *observer = new ReducedMotionObserver;
  observer->state = state;
  id notification_observer = [workspace.notificationCenter
      addObserverForName:
          NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification
                  object:workspace
                   queue:[NSOperationQueue mainQueue]
              usingBlock:^(NSNotification *notification) {
                // Read the workspace from the notification rather than capturing it, which
                // would retain the notification center that owns this block.
                NSWorkspace *sender = notification.object;
                if (state->active.load(std::memory_order_acquire))
                  state->callback(
                      state->callback_data,
                      sender.accessibilityDisplayShouldReduceMotion);
              }];
  observer->notification_observer =
      (__bridge_retained void *)notification_observer;

  // Initialize the view immediately; later system changes arrive through the
  // workspace notification above.
  state->callback(state->callback_data,
                  workspace.accessibilityDisplayShouldReduceMotion);
  return observer;
}

extern "C" void photon_reduced_motion_observer_destroy(void *opaque_observer) {
  auto *observer = static_cast<ReducedMotionObserver *>(opaque_observer);
  if (!observer)
    return;

  observer->state->active.store(false, std::memory_order_release);
  if (observer->notification_observer) {
    id notification_observer = (__bridge id)observer->notification_observer;
    [[NSWorkspace sharedWorkspace].notificationCenter
        removeObserver:notification_observer];
    CFRelease(observer->notification_observer);
  }
  delete observer;
}
