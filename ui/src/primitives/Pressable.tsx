import type { Props, StyleDesc } from "@gpuix/react";
import type { ReactNode } from "react";
import type { InteractionColors } from "../theme/colors";
import { useThemeColors } from "../theme/ThemeProvider";
import { borders, space } from "../theme/tokens";

/**
 * The interaction contract every control in Photon shares.
 *
 * A component does not track hover or press in React state. It calls
 * `pressableStyle` for its geometry and declares a variant, and GPUI applies
 * the state styles natively: moving the pointer over a button never crosses
 * into JavaScript, so hover costs no React render and no bridge call.
 *
 * States owned here:
 *   normal   `backgroundColor`, `color`, `borderColor`
 *   hover    `style.hover`
 *   pressed  `style.active`
 *   selected `style.active`, so pressing a selected item still reads as selected
 *   focus    `style.focusVisible`, an outline that takes no layout space
 *   disabled `opacity` plus the disabled foreground, and the caller drops the
 *            tab stop and the click
 */
export interface PressableStyleOptions {
  disabled?: boolean;
  /**
   * Which interaction palette to read from the active theme. `ghost` is the
   * top-level palette, so a control that only needs a hover wash needs no
   * variant at all.
   */
  variant?: "ghost" | "subtle" | "solid";
  /** Persistent fill for the chosen item in a strip or a menu. */
  selected?: boolean;
  /** Set when the control paints its own border. */
  borderWidth?: number;
  borderRadius?: number;
  /** Siblings of a control, e.g. toolbar buttons. */
  gap?: number;
  disabledOpacity?: number;
}

/** Read one variant's palette from the active theme colours. */
export function useInteractionPalette(
  variant: PressableStyleOptions["variant"],
): InteractionColors {
  const colors = useThemeColors();
  if (variant === "solid") return colors.solid;
  if (variant === "subtle") return colors.subtle;
  return colors;
}

/**
 * Build the style for a pressable surface.
 *
 * The result is a plain `StyleDesc`, so it composes into any host element and
 * stays cheap to memoise.
 */
export function pressableStyle({
  variant = "ghost",
  selected = false,
  disabled = false,
  borderWidth,
  borderRadius,
  gap = space.sm,
  disabledOpacity = 0.45,
}: PressableStyleOptions = {}): StyleDesc {
  const colors = useThemeColors();
  const palette =
    variant === "solid"
      ? colors.solid
      : variant === "subtle"
        ? colors.subtle
        : colors;
  const bordered = borderWidth !== undefined && borderWidth > 0;

  return {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap,
    backgroundColor: selected ? palette.bgSelected : palette.bg,
    color: disabled ? colors.disabledFg : palette.fg,
    borderWidth,
    borderColor: bordered
      ? (palette.border ?? colors.surfaceBorder)
      : undefined,
    borderRadius,
    opacity: disabled ? disabledOpacity : 1,
    cursor: disabled ? "not-allowed" : "pointer",
    userSelect: "none",
    // Resolved by GPUI against its own element state; nothing reaches JS.
    hover: disabled
      ? undefined
      : {
          backgroundColor: palette.bgHover,
          color: palette.fgHover,
          borderColor: bordered
            ? (palette.borderHover ?? colors.surfaceBorder)
            : undefined,
        },
    active: disabled
      ? undefined
      : {
          backgroundColor: selected ? palette.bgSelected : palette.bgPressed,
          color: palette.fgPressed,
        },
    focusVisible: disabled
      ? undefined
      : {
          outlineWidth: borders.thick,
          outlineColor: colors.focusRing,
        },
  };
}

export interface CenterProps extends Props {
  children?: ReactNode;
}

/** A flex row with both axes centred. The most repeated shape in the shell. */
export function Center({ children, style, ...props }: CenterProps) {
  return (
    <div
      {...props}
      style={{
        display: "flex",
        flexDirection: "row",
        alignItems: "center",
        justifyContent: "center",
        ...style,
      }}
    >
      {children}
    </div>
  );
}
