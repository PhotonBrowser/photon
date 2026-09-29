import type { StyleDesc } from "@gpuix/react";

export interface PhotonWebViewProps {
  url: string;
  style?: StyleDesc;
}

export function PhotonWebView({ url, style }: PhotonWebViewProps) {
  return <photon-webview url={url} style={style} />;
}
