import type { Props, StyleDesc } from "@gpuix/react";
import type { ReactNode } from "react";
import type { ThemeColors } from "../theme/colors";
import type { Elevation } from "../theme/shadow";
import { shadow } from "../theme/shadow";
import { useTheme } from "../theme/ThemeProvider";
import { borders, radii, space } from "../theme/tokens";

export type SurfaceVariant =
  | "transparent"
  | "chrome"
  | "panel"
  | "menu"
  | "tooltip";

type ColorKey = {
  [K in keyof ThemeColors]: ThemeColors[K] extends string | undefined
    ? K
    : never;
}[keyof ThemeColors];

interface SurfaceSpec {
  backgroundColor?: ColorKey;
  borderColor?: ColorKey;
  borderWidth?: number;
  borderRadius?: number;
  padding?: number;
  elevation?: Elevation;
}

/**
 * Background, border, radius and elevation per surface role, so a menu and a
 * page never disagree about how far apart they are.
 */
const SURFACES = {
  /** No fill of its own: whatever is behind it shows through. */
  transparent: {},
  /** Window chrome: translucent, no border, no shadow. */
  chrome: {
    backgroundColor: "windowBg",
    borderRadius: radii.none,
    padding: 0,
  },
  /** A page or content frame. */
  panel: {
    backgroundColor: "windowBg",
    borderWidth: borders.hairline,
    borderColor: "surfaceBorder",
    borderRadius: radii.md,
  },
  /** An overlay list: menus and popovers. */
  menu: {
    backgroundColor: "surfaceBg",
    borderWidth: borders.hairline,
    borderColor: "surfaceBorder",
    borderRadius: radii.xl,
    padding: space.xs,
    elevation: "overlay",
  },
  /** A hint attached to a control. */
  tooltip: {
    backgroundColor: "surfaceBg",
    borderWidth: borders.hairline,
    borderColor: "surfaceBorder",
    borderRadius: radii.md,
    padding: space.xs,
    elevation: "raised",
  },
} as const satisfies Record<SurfaceVariant, SurfaceSpec>;

export interface SurfaceProps extends Omit<Props, "style"> {
  variant?: SurfaceVariant;
  /** Merge over the variant, for layout and one-off geometry. */
  style?: StyleDesc;
  children?: ReactNode;
}

/**
 * A background with a role.
 *
 * Every filled or positioned surface in Photon is one of these, which is also
 * what keeps overlay hit-testing correct: GPUI blocks the wheel behind a
 * painted fill, and the overlay variants are the ones that should.
 */
export function Surface({
  variant = "panel",
  style,
  children,
  ...props
}: SurfaceProps) {
  const { colors } = useTheme();
  const spec: SurfaceSpec = SURFACES[variant];
  const boxShadow = spec.elevation ? shadow(colors, spec.elevation) : undefined;

  return (
    <div
      {...props}
      style={{
        display: "flex",
        flexDirection: "column",
        backgroundColor:
          spec.backgroundColor === undefined
            ? undefined
            : colors[spec.backgroundColor],
        borderWidth: spec.borderWidth,
        borderColor:
          spec.borderColor === undefined ? undefined : colors[spec.borderColor],
        borderRadius: spec.borderRadius,
        padding: spec.padding,
        boxShadow,
        ...style,
      }}
    >
      {children}
    </div>
  );
}
