import type { StyleDesc } from "@gpuix/react";

declare module "@gpuix/react/jsx-runtime" {
  namespace JSX {
    interface IntrinsicElements {
      "photon-webview": {
        url: string;
        style?: StyleDesc;
      };
    }
  }
}
