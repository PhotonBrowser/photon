import type { KeyEvent, Props, StyleDesc } from "@gpuix/react";
import { motion } from "@gpuix/react";
import type { ReactElement, ReactNode } from "react";
import { Children, cloneElement, isValidElement } from "react";
import { useReducedMotion, useTransition } from "../motion";
import { pressableStyle } from "../primitives/Pressable";
import { useThemeColors } from "../theme";
import { borders, fontSize, radii, sizes, space } from "../theme/tokens";

export interface TabProps extends Omit<Props, "style"> {
  /** Stable identity for selection, also the accessibility value. */
  value: string;
  selected?: boolean;
  disabled?: boolean;
  /** Favicon or leading glyph, tinted by the control's foreground. */
  icon?: ReactNode;
  /** Visible label. Omit for an icon-only tab. */
  label?: string;
  /** Trailing node, e.g. a close control. */
  trailing?: ReactNode;
  onSelect?: (value: string) => void;
  style?: StyleDesc;
  testId?: string;
}

/**
 * One tab in a `TabStrip`.
 *
 * Hover and press are GPUI's own `hover` and `active` element states, so a
 * pointer move over a tab never reaches JavaScript.
 *
 * Selection is the one thing that has to move, so it moves natively: the wash
 * under the label is a `motion.div` whose opacity Rust tweens, and the selection
 * crossfades rather than snapping. The wash belongs to the tab instead of being
 * one bar that slides across the strip, because that needs no measurement of the
 * siblings, so a strip that resizes mid-transition cannot drift.
 */
export function Tab({
  value,
  selected = false,
  disabled = false,
  icon,
  label,
  trailing,
  onSelect,
  style,
  tabIndex,
  testId,
  ...props
}: TabProps) {
  const colors = useThemeColors();
  const reduced = useReducedMotion();
  const transition = useTransition({ duration: "quick", easing: "emphasized" });

  return (
    <div
      {...props}
      role="tab"
      aria-selected={selected}
      aria-disabled={disabled || undefined}
      tabIndex={tabIndex ?? (selected ? 0 : -1)}
      testId={testId}
      onClick={disabled ? undefined : () => onSelect?.(value)}
      onKeyDown={(event: KeyEvent) => {
        if (disabled) return;
        if (event.key === "enter" && !event.isHeld) onSelect?.(value);
        if (event.key === "space") event.preventDefault();
      }}
      onKeyUp={(event: KeyEvent) => {
        if (!disabled && event.key === "space" && !event.defaultPrevented) {
          onSelect?.(value);
        }
      }}
      style={{
        ...pressableStyle({ variant: "ghost", borderRadius: radii.lg }),
        position: "relative",
        flexDirection: "row",
        justifyContent: "flex-start",
        gap: space.sm,
        flexGrow: 1,
        flexShrink: 1,
        flexBasis: 0,
        minWidth: sizes.tabMinWidth,
        maxWidth: sizes.tabMaxWidth,
        height: sizes.tabHeight,
        paddingLeft: space.md,
        paddingRight: space.md,
        // The selection wash animates opacity natively. Hover and press are
        // native GPUI state styles on this hit target.
        backgroundColor: "transparent",
        color: disabled
          ? colors.disabledFg
          : selected
            ? colors.text
            : colors.fg,
        opacity: disabled ? 0.45 : 1,
        cursor: disabled ? "not-allowed" : "pointer",
        hover: disabled ? undefined : { backgroundColor: colors.bgHover },
        active: disabled
          ? undefined
          : {
              backgroundColor: selected ? colors.bgSelected : colors.bgPressed,
            },
        focusVisible: disabled
          ? undefined
          : {
              outlineWidth: borders.thick,
              outlineColor: colors.focusRing,
            },
        overflow: "hidden",
        fontSize: fontSize.body,
        whiteSpace: "nowrap",
        ...style,
      }}
    >
      <motion.div
        initial={reduced ? false : { opacity: selected ? 1 : 0 }}
        animate={{ opacity: selected ? 1 : 0 }}
        transition={transition}
        style={{
          position: "absolute",
          top: space.none,
          right: space.none,
          bottom: space.none,
          left: space.none,
          borderRadius: radii.lg,
          backgroundColor: colors.bgSelected,
          // The wash is decoration: it must not take the tab's hits.
          pointerEvents: "none",
        }}
      />
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
      {label && (
        <div
          style={{
            flexGrow: 1,
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {label}
        </div>
      )}
      {trailing}
    </div>
  );
}

export interface TabStripProps extends Omit<Props, "style"> {
  /** The selected tab's `value`. */
  selected?: string;
  onSelect?: (value: string) => void;
  style?: StyleDesc;
  children?: ReactNode;
}

/**
 * The row of tabs.
 *
 * It owns the container look and the keyboard model: one tab stop at a time, and
 * arrow keys, Home and End moving the selection. Selection state itself stays
 * with the caller, because which tab is current is browser state, not layout.
 */
export function TabStrip({
  selected,
  onSelect,
  style,
  children,
  ...props
}: TabStripProps) {
  const colors = useThemeColors();
  const values = tabValues(children);

  /** Move the selection, wrapping at both ends like a native tab strip. */
  const selectAt = (position: number) => {
    if (values.length === 0) return;
    const wrapped =
      ((position % values.length) + values.length) % values.length;
    const value = values[wrapped];
    if (value !== undefined) onSelect?.(value);
  };

  return (
    <div
      {...props}
      role="tablist"
      style={{
        display: "flex",
        flexDirection: "row",
        alignItems: "center",
        flexShrink: 0,
        gap: space.xxs,
        padding: space.xxs,
        borderRadius: radii.xl,
        backgroundColor: colors.subtle.bg,
        borderWidth: borders.hairline,
        borderColor: colors.surfaceBorder,
        overflow: "hidden",
        ...style,
      }}
    >
      {Children.map(children, (child) =>
        isValidElement<TabProps>(child) && child.props.value !== undefined
          ? cloneElement(child as ReactElement<TabProps>, {
              selected: child.props.selected ?? child.props.value === selected,
              onSelect: child.props.onSelect ?? onSelect,
              onKeyDown: (event: KeyEvent) => {
                child.props.onKeyDown?.(event);
                if (event.defaultPrevented) return;
                const at = values.indexOf(child.props.value ?? "");
                if (event.key === "arrowleft") selectAt(at - 1);
                else if (event.key === "arrowright") selectAt(at + 1);
                else if (event.key === "home") selectAt(0);
                else if (event.key === "end") selectAt(values.length - 1);
              },
            })
          : child,
      )}
    </div>
  );
}

/** Read the ordered `value` of each tab child, for arrow-key movement. */
function tabValues(children: ReactNode): string[] {
  const values: string[] = [];
  Children.forEach(children, (child) => {
    if (
      isValidElement<TabProps>(child) &&
      typeof child.props.value === "string" &&
      !child.props.disabled
    ) {
      values.push(child.props.value);
    }
  });
  return values;
}
