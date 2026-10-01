import { useThemeColors } from "../theme";
import { radii, space } from "../theme/tokens";
import { PhotonWebView } from "./PhotonWebView";

export interface BrowserContentProps {
  url: string;
}

/**
 * The page area.
 *
 * It is the frame around the page: padding, background and the rounded clip.
 * Nothing here knows how a page is navigated or what state it is in, so the
 * same component serves a single page and a tabbed view.
 */
export function BrowserContent({ url }: BrowserContentProps) {
  const colors = useThemeColors();

  return (
    <div
      style={{
        display: "flex",
        flexGrow: 1,
        minHeight: 0,
        padding: space.xs,
        backgroundColor: colors.windowBg,
        // The chrome sits flush against the top of the page.
        paddingTop: space.none,
      }}
    >
      <PhotonWebView
        url={url}
        style={{
          flexGrow: 1,
          minWidth: 0,
          minHeight: 0,
          backgroundColor: "transparent",
          borderRadius: radii.md,
          overflow: "hidden",
        }}
      />
    </div>
  );
}
