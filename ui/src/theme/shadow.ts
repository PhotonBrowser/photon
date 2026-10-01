import type { BoxShadow } from "@gpuix/native/host";
import type { GpuixTheme } from "@gpuix/react";
import type { ThemeAppearance, ThemeColors } from "./colors";

/** Depth levels. Shadows are the only elevation cue; borders handle the rest. */
export type Elevation = "rest" | "raised" | "overlay";

const SHADOWS = {
  rest: undefined,
  raised: { offsetX: 0, offsetY: 2, blurRadius: 8, spreadRadius: -2 },
  overlay: { offsetX: 0, offsetY: 10, blurRadius: 30, spreadRadius: -6 },
} as const satisfies Record<Elevation, Omit<BoxShadow, "color"> | undefined>;

/**
 * The shadow for an elevation level, tinted from the active appearance so a
 * dark window never paints a black shadow on a black surface.
 */
export function shadow(
  colors: ThemeColors,
  level: Elevation,
): BoxShadow | undefined {
  const shape = SHADOWS[level];
  return shape === undefined
    ? undefined
    : { ...shape, color: colors.surfaceShadow };
}

/**
 * Colours for a native `<input>`.
 *
 * The text field paints its own caret, selection and placeholder, so it takes
 * the tokens it needs rather than inheriting from the surrounding React style.
 */
export function inputTheme(
  colors: ThemeColors,
  appearance: ThemeAppearance,
): GpuixTheme {
  return {
    appearance,
    bg: colors.bg,
    border: colors.border,
    text: colors.fg,
    textMuted: colors.textMuted,
    caret: colors.accent,
  };
}
