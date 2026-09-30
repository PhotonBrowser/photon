import { PhotonWebView } from "./PhotonWebView";
import { theme } from "../theme";

interface WebViewSurfaceProps {
  url: string;
}

export function WebViewSurface({ url }: WebViewSurfaceProps) {
  return (
    <div
      style={{
        flexGrow: 1,
        minHeight: 0,
        paddingLeft: theme.space.xs,
        paddingRight: theme.space.xs,
        paddingBottom: theme.space.xs,
      }}
    >
      <PhotonWebView
        url={url}
        style={{
          width: "100%",
          height: "100%",
          borderRadius: theme.radius.md,
          overflow: "hidden",
        }}
      />
    </div>
  );
}
