/** Shared visual values for the Photon shell. Keep repeated UI styling here. */
export const themes = {
  dark: {
    color: {
      window: "oklch(0.12 0.012 265 / 0.45)",
      titlebarText: "oklch(0.72 0.012 265)",
      button: "oklch(0.24 0.012 265 / 0.9)",
      buttonHover: "oklch(0.3 0.012 265 / 0.8)",
      buttonPressed: "oklch(0.34 0.012 265 / 0.82)",
      icon: "oklch(0.78 0.012 265)",
    },
  },
  light: {
    color: {
      window: "oklch(0.97 0.008 265 / 0.88)",
      titlebarText: "oklch(0.32 0.012 265)",
      button: "oklch(0.9 0.012 265 / 0.9)",
      buttonHover: "oklch(0.85 0.012 265)",
      buttonPressed: "oklch(0.82 0.012 265 / 0.9)",
      icon: "oklch(0.35 0.012 265)",
    },
  },
} as const;

export type ThemeAppearance = keyof typeof themes;
export type ThemeMode = ThemeAppearance | "system";
export type Theme = (typeof themes)[ThemeAppearance];

export const theme = {
  ...themes.dark,
  space: {
    none: 0,
    xxs: 2,
    xs: 4,
    sm: 8,
    md: 12,
    lg: 16,
  },
  radius: {
    none: 0,
    sm: 4,
    md: 6,
  },
  typography: {
    titlebar: 13,
  },
  layout: {
    titlebarControlInset: 12,
    titlebarActionInset: 6,
    titlebarControlSize: 14,
    titlebarVerticalPadding: 4,
    titlebarActionSize: 26,
  },
} as const;
