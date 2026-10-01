import type { Props, StyleDesc } from "@gpuix/react";
import type { ReactNode } from "react";
import { useThemeColors } from "../theme/ThemeProvider";
import { layout, space } from "../theme/tokens";

export type ToolbarVariant = "chrome" | "panel";

export interface ToolbarProps extends Omit<Props, "style"> {
  variant?: ToolbarVariant;
  /** Merge over the variant, for padding and gaps. */
  style?: StyleDesc;
  children?: ReactNode;
}

/**
 * A full-width row of controls.
 *
 * It owns the row's alignment, height, background and the gap between its
 * children, so a toolbar is a container for controls rather than a pile of
 * flexbox.
 */
export function Toolbar({
  variant = "chrome",
  style,
  children,
  ...props
}: ToolbarProps) {
  const colors = useThemeColors();
  return (
    <div
      {...props}
      style={{
        display: "flex",
        flexDirection: "row",
        flexShrink: 0,
        alignItems: "center",
        justifyContent: "flex-start",
        gap: space.sm,
        minHeight: layout.titlebarHeight,
        paddingTop: layout.chromePaddingY,
        paddingBottom: layout.chromePaddingY,
        paddingLeft: space.sm,
        paddingRight: space.sm,
        color: colors.text,
        backgroundColor:
          variant === "chrome" ? colors.windowBg : colors.surfaceBg,
        ...style,
      }}
    >
      {children}
    </div>
  );
}
