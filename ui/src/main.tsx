import { render } from "@gpuix/react";
import { App } from "./App";

console.info("Photon UI: GPUIX / GPUI");

render(<App />, {
  title: "Photon",
  width: 1100,
  height: 720,
  minWidth: 640,
  minHeight: 420,
  windowBackground: "opaque",
  appName: "Photon",
});
