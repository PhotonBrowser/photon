/**
 * Photon's colour tokens.
 *
 * Roles are semantic (`controlBgPressed`), not literal (`grey700`), so a light
 * and a dark appearance stay in step and a component never picks a colour by
 * guessing which one looks right.
 *
 * Every interactive control resolves its whole state set from one
 * {@link InteractionColors} entry: normal, hover, pressed, selected, plus the
 * foreground for each. Adding a control means picking a variant, not inventing
 * new colours.
 */

/** One component's full interaction state set, resolved from tokens alone. */
export interface InteractionColors {
  /** Resting fill. `transparent` for a ghost control. */
  bg: string;
  bgHover: string;
  bgPressed: string;
  /** Persistent fill for the selected item in a strip or menu. */
  bgSelected: string;
  /** Resting content colour: label text and glyphs. */
  fg: string;
  fgHover: string;
  fgPressed: string;
  /** Optional resting border. Omitted means the control has no border. */
  border?: string;
  borderHover?: string;
}

export interface ThemeColors extends InteractionColors {
  /** Window background behind everything. */
  windowBg: string;
  /** Elevated surfaces: menus, popovers, tooltips, panels. */
  surfaceBg: string;
  surfaceBorder: string;
  surfaceShadow: string;
  /** Wash behind a menu row under the keyboard cursor. */
  surfaceHover: string;
  /** Content colours for elevated surfaces. */
  text: string;
  textMuted: string;
  /** Foreground while the keyboard cursor is over a menu row. */
  textStrong: string;
  /** Accent, for selection marks and the caret. */
  accent: string;
  /** Ring drawn on `:focus-visible`. */
  focusRing: string;
  /** Foreground and fill of a disabled control. */
  disabledFg: string;
  disabledBg: string;
  /** Scrim behind a modal layer. */
  overlayWash: string;

  /** Tinted, low-contrast control: tab strips, toolbar groups. */
  subtle: InteractionColors;
  /** Solid control: the primary action in a dialog or menu. */
  solid: InteractionColors;
}

export type ThemeAppearance = "dark" | "light";

const dark: ThemeColors = {
  // Ghost controls: no resting fill, the window shows through.
  bg: "transparent",
  bgHover: "oklch(1 0 0 / 0.06)",
  bgPressed: "oklch(1 0 0 / 0.1)",
  bgSelected: "oklch(1 0 0 / 0.1)",
  fg: "oklch(0.78 0.012 265)",
  fgHover: "oklch(0.9 0.006 265)",
  fgPressed: "oklch(0.92 0.006 265)",

  windowBg: "oklch(0.12 0.012 265 / 0.18)",
  surfaceBg: "oklch(0.23 0.008 265)",
  surfaceBorder: "oklch(1 0 0 / 0.12)",
  surfaceShadow: "oklch(0 0 0 / 0.42)",
  surfaceHover: "oklch(1 0 0 / 0.06)",
  text: "oklch(0.92 0.006 265)",
  textMuted: "oklch(0.69 0.008 265)",
  textStrong: "oklch(0.97 0.004 265)",
  accent: "oklch(0.92 0.006 265)",
  focusRing: "oklch(0.78 0.012 265)",
  disabledFg: "oklch(0.55 0.008 265)",
  disabledBg: "oklch(1 0 0 / 0.03)",
  overlayWash: "oklch(0 0 0 / 0.42)",

  subtle: {
    bg: "oklch(1 0 0 / 0.04)",
    bgHover: "oklch(1 0 0 / 0.08)",
    bgPressed: "oklch(1 0 0 / 0.12)",
    bgSelected: "oklch(1 0 0 / 0.14)",
    fg: "oklch(0.78 0.012 265)",
    fgHover: "oklch(0.9 0.006 265)",
    fgPressed: "oklch(0.92 0.006 265)",
  },

  solid: {
    bg: "oklch(0.9 0.006 265)",
    bgHover: "oklch(0.97 0.004 265)",
    bgPressed: "oklch(0.78 0.012 265)",
    bgSelected: "oklch(0.9 0.006 265)",
    fg: "oklch(0.2 0.008 265)",
    fgHover: "oklch(0.15 0.008 265)",
    fgPressed: "oklch(0.25 0.008 265)",
  },
};

const light: ThemeColors = {
  bg: "transparent",
  bgHover: "oklch(0 0 0 / 0.035)",
  bgPressed: "oklch(0 0 0 / 0.06)",
  bgSelected: "oklch(0 0 0 / 0.06)",
  fg: "oklch(0.35 0.012 265)",
  fgHover: "oklch(0.2 0.008 265)",
  fgPressed: "oklch(0.15 0.008 265)",

  windowBg: "oklch(0.97 0.008 265 / 0.32)",
  surfaceBg: "oklch(0.99 0.003 265)",
  surfaceBorder: "oklch(0 0 0 / 0.1)",
  surfaceShadow: "oklch(0 0 0 / 0.18)",
  surfaceHover: "oklch(0 0 0 / 0.035)",
  text: "oklch(0.25 0.008 265)",
  textMuted: "oklch(0.5 0.008 265)",
  textStrong: "oklch(0.2 0.008 265)",
  accent: "oklch(0.25 0.008 265)",
  focusRing: "oklch(0.35 0.012 265)",
  disabledFg: "oklch(0.62 0.008 265)",
  disabledBg: "oklch(0 0 0 / 0.02)",
  overlayWash: "oklch(0 0 0 / 0.18)",

  subtle: {
    bg: "oklch(0 0 0 / 0.03)",
    bgHover: "oklch(0 0 0 / 0.06)",
    bgPressed: "oklch(0 0 0 / 0.09)",
    bgSelected: "oklch(0 0 0 / 0.1)",
    fg: "oklch(0.35 0.012 265)",
    fgHover: "oklch(0.2 0.008 265)",
    fgPressed: "oklch(0.15 0.008 265)",
  },

  solid: {
    bg: "oklch(0.25 0.008 265)",
    bgHover: "oklch(0.18 0.008 265)",
    bgPressed: "oklch(0.12 0.012 265)",
    bgSelected: "oklch(0.25 0.008 265)",
    fg: "oklch(0.99 0.003 265)",
    fgHover: "oklch(1 0 0)",
    fgPressed: "oklch(0.95 0.003 265)",
  },
};

export const themes = { dark, light } satisfies Record<
  ThemeAppearance,
  ThemeColors
>;
