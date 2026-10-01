import type { Props, SelectItemProps, StyleDesc } from "@gpuix/react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
} from "@gpuix/react";
import type { ReactNode } from "react";
import { overlayCollisionPadding, overlayOffset } from "../motion";
import { pressableStyle } from "../primitives/Pressable";
import { useThemeColors } from "../theme";
import { shadow } from "../theme/shadow";
import {
  borders,
  fontSize,
  fontWeight,
  radii,
  sizes,
  space,
} from "../theme/tokens";

export interface MenuProps {
  /** The selected item's value. */
  value?: string;
  onValueChange?: (value: string) => void;
  children?: ReactNode;
}

/**
 * A list of choices, built on GPUIX's headless `Select`.
 *
 * GPUIX supplies the anchored layer, the keyboard cursor, Escape, click-outside
 * dismissal and focus restoration. This supplies the surface tokens and the row
 * states, so every menu in Photon looks and behaves the same and no caller has
 * to assemble a floating card by hand.
 */
export function Menu({ value, onValueChange, children }: MenuProps) {
  return (
    <Select value={value} onValueChange={onValueChange}>
      {children}
    </Select>
  );
}

export interface MenuTriggerProps extends Omit<Props, "style"> {
  /**
   * The control that opens the menu. Usually a `Button` or `IconButton` passed
   * with `asChild`, so it keeps its own look and stays one control.
   */
  asChild?: boolean;
  disabled?: boolean;
  style?: Props["style"];
}

/**
 * The menu's trigger.
 *
 * With `asChild` the menu takes over the child's press, focus and
 * `aria-expanded` and adds no style of its own, so a trigger is one control
 * rather than a control wrapped in another.
 */
export function MenuTrigger({
  asChild = true,
  disabled,
  style,
  ...props
}: MenuTriggerProps) {
  return (
    <SelectTrigger
      {...props}
      asChild={asChild}
      disabled={disabled}
      role="button"
      aria-haspopup="listbox"
      style={
        style ??
        (asChild
          ? undefined
          : {
              ...pressableStyle({ variant: "ghost", borderRadius: radii.md }),
            })
      }
    />
  );
}

export interface MenuContentProps extends Omit<Props, "style"> {
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  sideOffset?: number;
  /** Minimum width, for menus whose labels are short. */
  minWidth?: number;
  style?: Props["style"];
  children?: ReactNode;
}

/**
 * The floating list.
 *
 * It must be an anchored layer, not a positioned card: GPUIX paints anchors above
 * ordinary content and blocks the wheel behind them, which is what keeps a menu
 * above a scrolling page and keeps a click from reaching the text underneath.
 */
export function MenuContent({
  align = "start",
  side = "bottom",
  sideOffset = overlayOffset.default,
  minWidth = sizes.menuMinWidth,
  style,
  children,
  ...props
}: MenuContentProps) {
  const colors = useThemeColors();
  const surface: StyleDesc = {
    display: "flex",
    flexDirection: "column",
    gap: space.xxs,
    minWidth,
    padding: space.xs,
    backgroundColor: colors.surfaceBg,
    borderWidth: borders.hairline,
    borderColor: colors.surfaceBorder,
    borderRadius: radii.xl,
    color: colors.text,
    boxShadow: shadow(colors, "overlay"),
  };

  return (
    <SelectContent
      {...props}
      role="listbox"
      align={align}
      side={side}
      sideOffset={sideOffset}
      collisionPadding={overlayCollisionPadding}
      style={{ ...surface, ...(style as StyleDesc | undefined) }}
    >
      {children}
    </SelectContent>
  );
}

export interface MenuItemProps extends Omit<Props, "style"> {
  /** Merge over the row tokens, for a one-off width or a swatch. */
  style?: SelectItemProps["style"];
  value: string;
  disabled?: boolean;
  /** Leading glyph, tinted by the row's foreground. */
  icon?: ReactNode;
  description?: string;
  /** Selection mark, shown on the right of the chosen item. */
  selectedMark?: ReactNode;
  children?: ReactNode;
}

/**
 * One row.
 *
 * The row owns every state it can reach from the palette: resting, hovered,
 * pressed, selected and disabled. Highlighted is GPUIX's keyboard cursor, so
 * arrowing through a menu repaints one row natively instead of re-rendering the
 * list, and it is combined with selection rather than replacing it.
 */
export function MenuItem({
  value,
  disabled = false,
  icon,
  description,
  selectedMark,
  children,
  style,
  ...props
}: MenuItemProps) {
  const colors = useThemeColors();

  return (
    <SelectItem
      {...props}
      value={value}
      disabled={disabled}
      role="option"
      style={(state) => ({
        display: "flex",
        flexDirection: "row",
        alignItems: "center",
        gap: space.sm,
        minHeight: sizes.menuItemHeight,
        paddingLeft: space.sm,
        paddingRight: space.sm,
        borderRadius: radii.lg,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.45 : 1,
        color: disabled
          ? colors.disabledFg
          : state.highlighted
            ? colors.textStrong
            : colors.text,
        backgroundColor: disabled
          ? undefined
          : state.selected
            ? colors.bgSelected
            : state.highlighted
              ? colors.surfaceHover
              : undefined,
        ...(typeof style === "function" ? style(state) : style),
      })}
    >
      {({ selected }) => (
        <>
          {icon && (
            <div
              style={{
                display: "flex",
                flexDirection: "row",
                alignItems: "center",
                justifyContent: "center",
                width: sizes.iconSm,
                height: sizes.iconSm,
                flexShrink: 0,
              }}
            >
              {icon}
            </div>
          )}
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              flexGrow: 1,
              minWidth: 0,
              gap: space.xxs,
            }}
          >
            <div style={{ fontSize: fontSize.body }}>{children}</div>
            {description && (
              <div
                style={{ fontSize: fontSize.caption, color: colors.textMuted }}
              >
                {description}
              </div>
            )}
          </div>
          {selected && selectedMark && (
            <div
              style={{
                display: "flex",
                flexDirection: "row",
                alignItems: "center",
                justifyContent: "center",
                // The mark is a glyph sized by its box: a mark that fills this
                // box collapses to zero without an explicit size, and gpui
                // refuses to paint a zero-sized SVG.
                width: sizes.iconSm,
                height: sizes.iconSm,
                flexShrink: 0,
                color: colors.accent,
              }}
            >
              {selectedMark}
            </div>
          )}
        </>
      )}
    </SelectItem>
  );
}

export interface MenuLabelProps extends Props {
  children?: ReactNode;
}

/** A non-interactive heading, for grouping rows in a long menu. */
export function MenuLabel({ children, style, ...props }: MenuLabelProps) {
  const colors = useThemeColors();
  return (
    <SelectLabel
      {...props}
      style={{
        fontSize: fontSize.caption,
        fontWeight: fontWeight.semibold,
        color: colors.textMuted,
        paddingLeft: space.sm,
        paddingRight: space.sm,
        paddingTop: space.sm,
        paddingBottom: space.xs,
        ...(style as StyleDesc | undefined),
      }}
    >
      {children}
    </SelectLabel>
  );
}

export interface MenuSeparatorProps extends Props {}

/** A hairline between groups of rows. */
export function MenuSeparator({ style, ...props }: MenuSeparatorProps) {
  const colors = useThemeColors();
  return (
    <SelectSeparator
      {...props}
      style={{
        height: 1,
        marginTop: space.xs,
        marginBottom: space.xs,
        backgroundColor: colors.surfaceBorder,
        ...(style as StyleDesc | undefined),
      }}
    />
  );
}
