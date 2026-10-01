import type { Props, StyleDesc } from "@gpuix/react";
import { Button as GpuixButton } from "@gpuix/react";
import type { ReactNode } from "react";
import { pressableStyle } from "../primitives/Pressable";
import { fontSize, radii, sizes, space } from "../theme/tokens";

export type ControlVariant = "ghost" | "subtle" | "solid";
export type ControlSize = "sm" | "md" | "lg";

/**
 * Control geometry by size. Box, glyph and label scale together, so a control is
 * never a small box holding a large icon.
 */
const CONTROL_METRICS = {
  sm: {
    box: sizes.controlSm,
    icon: sizes.iconSm,
    padding: space.xs,
    radius: radii.md,
    fontSize: fontSize.small,
  },
  md: {
    box: sizes.controlMd,
    icon: sizes.iconMd,
    padding: space.sm,
    radius: radii.md,
    fontSize: fontSize.body,
  },
  lg: {
    box: sizes.controlLg,
    icon: sizes.iconLg,
    padding: space.md,
    radius: radii.lg,
    fontSize: fontSize.title,
  },
} as const satisfies Record<
  ControlSize,
  {
    box: number;
    icon: number;
    padding: number;
    radius: number;
    fontSize: number;
  }
>;

export interface BaseControlProps {
  /** Which interaction palette to read from the active theme. */
  variant?: ControlVariant;
  size?: ControlSize;
  disabled?: boolean;
  /** Keep a disabled control in the tab order, for example while it saves. */
  focusableWhenDisabled?: boolean;
  /** Merged over the variant style, for layout only. */
  style?: StyleDesc;
  testId?: string;
}

/**
 * The pressable shell shared by `Button` and `IconButton`.
 *
 * Role, tab stop, keyboard activation and disabled behaviour all come from
 * GPUIX's `Button`; the state styles come from `pressableStyle`. The remaining
 * props are forwarded, so a control still works as the `asChild` child of a menu
 * trigger or a tooltip trigger.
 */
function Control({
  variant = "subtle",
  size = "md",
  disabled = false,
  focusableWhenDisabled,
  style,
  children,
  ...props
}: BaseControlProps & Omit<Props, "style"> & { children?: ReactNode }) {
  return (
    <GpuixButton
      {...props}
      disabled={disabled}
      focusableWhenDisabled={focusableWhenDisabled}
      style={{
        ...pressableStyle({
          variant,
          borderRadius: CONTROL_METRICS[size].radius,
        }),
        ...style,
      }}
    >
      {children}
    </GpuixButton>
  );
}

export interface ButtonProps extends BaseControlProps {
  children: ReactNode;
  onClick?: Props["onClick"];
}

/** A labelled control: text, optionally preceded by a glyph from the caller. */
export function Button({
  children,
  variant,
  size = "md",
  disabled,
  focusableWhenDisabled,
  style,
  testId,
  ...props
}: ButtonProps) {
  const metrics = CONTROL_METRICS[size];
  return (
    <Control
      variant={variant}
      size={size}
      disabled={disabled}
      focusableWhenDisabled={focusableWhenDisabled}
      style={{
        padding: metrics.padding,
        minHeight: metrics.box,
        ...style,
      }}
      testId={testId}
      {...props}
    >
      <div
        style={{
          fontSize: metrics.fontSize,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {children}
      </div>
    </Control>
  );
}

export interface IconButtonProps extends BaseControlProps {
  /** A tintable `<svg>`, or any node that fills the glyph box. */
  icon: ReactNode;
  /** Accessible name. Required: an icon-only control shows no text. */
  label: string;
  onClick?: Props["onClick"];
}

/**
 * An icon-only control.
 *
 * The glyph box is tinted with `color`, which inherits the control's state
 * colours, so an icon needs no separate hover or disabled artwork.
 */
export function IconButton({
  icon,
  label,
  variant,
  size = "md",
  disabled,
  focusableWhenDisabled,
  style,
  testId,
  ...props
}: IconButtonProps) {
  const metrics = CONTROL_METRICS[size];
  return (
    <Control
      variant={variant}
      size={size}
      disabled={disabled}
      focusableWhenDisabled={focusableWhenDisabled}
      style={{
        width: metrics.box,
        height: metrics.box,
        minWidth: metrics.box,
        minHeight: metrics.box,
        padding: 0,
        ...style,
      }}
      aria-label={label}
      testId={testId}
      {...props}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "center",
          width: metrics.icon,
          height: metrics.icon,
          flexShrink: 0,
        }}
      >
        {icon}
      </div>
    </Control>
  );
}
