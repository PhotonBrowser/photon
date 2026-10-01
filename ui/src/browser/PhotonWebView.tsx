import type { StyleDesc } from "@gpuix/react";

export interface PhotonWebViewProps {
  /**
   * What to open: an address, or plain text to search for. Which of the two it
   * is, and how it becomes a URL, is decided in Rust by `crates/photon-omnibox`
   * at the navigation boundary, so this prop never needs to be pre-resolved.
   */
  url: string;
  style?: StyleDesc;
}

/**
 * The engine surface.
 *
 * This is the whole integration with PhotonWebView: one custom element with a
 * url and a style. Frame bytes never reach JavaScript, so there is nothing else
 * to pass and nothing here can be reimplemented from the UI side.
 */
export function PhotonWebView({ url, style }: PhotonWebViewProps) {
  return <photon-webview url={url} style={style} />;
}
