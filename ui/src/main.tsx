import { render } from "@gpuix/react";
import { App } from "./App";
import { TRAFFIC_LIGHT_X, TRAFFIC_LIGHT_Y } from "./windowLayout";

console.info("Photon UI: GPUIX / GPUI");

render(<App />, {
  title: "Photon",
  width: 1100,
  height: 720,
  minWidth: 640,
  minHeight: 420,
  windowBackground: "blurred",
  titlebarTransparent: true,
  // Let AppKit handle titlebar dragging and double-click zoom/restore.
  appOwnsTitlebarDrag: false,
  // Position the native controls using the titlebar geometry from the theme.
  trafficLightX: TRAFFIC_LIGHT_X,
  trafficLightY: TRAFFIC_LIGHT_Y,
  appName: "Photon",
});
