import type { EventPayload, Props, StyleDesc } from "@gpuix/react";
import type { ReactNode } from "react";
import { useTheme } from "../theme";
import { inputTheme } from "../theme/shadow";
import { borders, fontSize, radii, sizes, space } from "../theme/tokens";
import { Surface } from "./Surface";

export interface AddressBarProps {
  /**
   * The current text. Omit to let the native field own its contents, which is
   * what a field that is never rewritten from state wants.
   */
  value?: string;
  placeholder?: string;
  /** Enter, or the go control. */
  onSubmit?: (value: string) => void;
  /** Every keystroke, for search-as-you-type. */
  onValueChange?: (value: string) => void;
  disabled?: boolean;
  /** Read-only field, e.g. while a page is loading. */
  readOnly?: boolean;
  /** Take keyboard focus on mount. */
  autoFocus?: boolean;
  /** Leading slot, normally a security or site icon. */
  leading?: ReactNode;
  /** Trailing slot, normally a reload or stop control. */
  trailing?: ReactNode;
  /** While loading, dim the field and show the trailing slot. */
  loading?: boolean;
  testId?: string;
  /** Merge over the field's geometry, for the width it takes in a toolbar. */
  style?: StyleDesc;
  inputProps?: Omit<Props, "style" | "value">;
}

/**
 * The single text field of the chrome.
 *
 * The field is a native GPUIX `<input>`, so the caret, IME, clipboard and
 * selection are the platform's. Focus is the one state the component owns, and
 * it is GPUI's: the ring is a `focusVisible` style on the field, which appears
 * for keyboard focus and not for a click, and it is an outline, so nothing
 * reflows when it appears.
 */
export function AddressBar({
  value,
  placeholder,
  onSubmit,
  onValueChange,
  disabled = false,
  readOnly = false,
  autoFocus = false,
  leading,
  trailing,
  loading = false,
  testId,
  style,
  inputProps,
}: AddressBarProps) {
  const { colors, appearance } = useTheme();
  const inactive = disabled || readOnly || loading;

  return (
    <Surface
      variant="transparent"
      style={{
        flexDirection: "row",
        alignItems: "center",
        flexGrow: 1,
        flexShrink: 1,
        minWidth: 0,
        gap: space.sm,
        height: sizes.addressBarMinHeight,
        paddingLeft: space.sm,
        paddingRight: space.xs,
        borderWidth: borders.hairline,
        borderColor: colors.surfaceBorder,
        borderRadius: radii.sm,
        backgroundColor: colors.subtle.bg,
        opacity: disabled ? 0.45 : 1,
        ...style,
      }}
    >
      {leading && (
        <div
          style={{
            display: "flex",
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
            color: colors.textMuted,
          }}
        >
          {leading}
        </div>
      )}
      <input
        {...inputProps}
        value={value}
        placeholder={placeholder}
        // GPUIX's input has no `disabled`; an inert field is a read-only one.
        readOnly={inactive}
        // biome-ignore lint/a11y/noAutofocus: opt-in prop; a browser address bar takes focus when its window opens
        autoFocus={autoFocus}
        theme={{
          ...inputTheme(colors, appearance),
          // The wrapper draws the address field border; the native editor's
          // border blends into its background in every focus state.
          border: colors.subtle.bg,
        }}
        testId={testId}
        onSubmit={(event: EventPayload) =>
          onSubmit?.(event.value ?? value ?? "")
        }
        onChange={(event: EventPayload) => onValueChange?.(event.value ?? "")}
        style={{
          flexGrow: 1,
          minWidth: 0,
          height: "100%",
          backgroundColor: "transparent",
          color: colors.fg,
          fontSize: fontSize.body,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          cursor: inactive ? "default" : "text",
          focusVisible: {
            // The surrounding address field owns the focus treatment. Keep
            // the native editor from painting its own selected border.
            outlineWidth: 0,
          },
        }}
      />
      {trailing && (
        <div
          style={{
            display: "flex",
            flexDirection: "row",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          {trailing}
        </div>
      )}
    </Surface>
  );
}
