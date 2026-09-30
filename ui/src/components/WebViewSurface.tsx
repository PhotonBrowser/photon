import { PhotonWebView } from "./PhotonWebView";
import { theme } from "../theme";

interface WebViewSurfaceProps {
  url: string;
}

export function WebViewSurface({ url }: WebViewSurfaceProps) {
  return (
    <div
      style={{
        display: "flex",
        flexGrow: 1,
        minHeight: 0,
        padding: theme.space.xs,
        paddingTop: theme.space.none,
      }}
    >
      <PhotonWebView
        url={url}
        style={{
          flexGrow: 1,
          minWidth: 0,
          minHeight: 0,
          backgroundColor: "transparent",
          borderRadius: theme.radius.md,
          overflow: "hidden",
        }}
      />
    </div>
  );
}
