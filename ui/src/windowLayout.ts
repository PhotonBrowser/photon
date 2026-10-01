import type { RenderOptions } from "@gpuix/react";
import { layout } from "./theme/tokens";

/**
 * Native window options.
 *
 * The traffic-light position derives from the same tokens the React titlebar
 * reserves space with, so the native controls can never drift out of the gap.
 */
export const TRAFFIC_LIGHT_X = layout.trafficLightInset;
export const TRAFFIC_LIGHT_Y =
  (layout.titlebarHeight - layout.trafficLightSize) / 2;

export const WINDOW_OPTIONS = {
  title: "Photon",
  width: 1100,
  height: 720,
  minWidth: 640,
  minHeight: 420,
  windowBackground: "blurred",
  titlebarTransparent: true,
  // Keep titlebar input in GPUI so clicks on app controls cannot drag or zoom
  // the native window before the controls receive them.
  appOwnsTitlebarDrag: true,
  trafficLightX: TRAFFIC_LIGHT_X,
  trafficLightY: TRAFFIC_LIGHT_Y,
  appName: "Photon",
  // GPUIX_BACKGROUND=1 opens the window behind whatever is active, for
  // automation runs that must not take the keyboard.
  focus: process.env.GPUIX_BACKGROUND !== "1",
} satisfies RenderOptions;
