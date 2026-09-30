import type { Theme } from "../theme";
import { WebViewSurface } from "./WebViewSurface";

interface WorkspaceProps {
  theme: Theme;
}

export function Workspace({ theme }: WorkspaceProps) {
  const url = process.env.PHOTON_URL ?? "https://example.com";

  return (
    <div
      style={{
        display: "flex",
        flexGrow: 1,
        minHeight: 0,
        backgroundColor: theme.color.window,
      }}
    >
      <WebViewSurface url={url} />
    </div>
  );
}
