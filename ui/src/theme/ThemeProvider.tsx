import type { ReactNode } from "react";
import { createContext, useContext } from "react";
import type { ThemeAppearance, ThemeColors } from "./colors";
import { themes } from "./colors";

/**
 * The active appearance.
 *
 * `mode` is what the user picked, `appearance` is what is on screen: the two
 * differ while the mode is `system`.
 */
export type ThemeMode = ThemeAppearance | "system";

export interface Theme {
  appearance: ThemeAppearance;
  colors: ThemeColors;
}

interface ThemeContextValue extends Theme {
  mode: ThemeMode;
  setMode: (mode: ThemeMode) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

export interface ThemeProviderProps {
  mode: ThemeMode;
  appearance: ThemeAppearance;
  setMode: (mode: ThemeMode) => void;
  children?: ReactNode;
}

/**
 * Makes the active colours reachable from any component, so a control declares
 * `variant="subtle"` instead of threading a colour object through every caller.
 */
export function ThemeProvider({
  mode,
  appearance,
  setMode,
  children,
}: ThemeProviderProps) {
  return (
    <ThemeContext.Provider
      value={{ mode, appearance, colors: themes[appearance], setMode }}
    >
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme(): ThemeContextValue {
  const value = useContext(ThemeContext);
  if (!value) throw new Error("useTheme must be used inside ThemeProvider");
  return value;
}

/** Convenience accessor for the common case: components only need colours. */
export function useThemeColors(): ThemeColors {
  return useTheme().colors;
}

export const THEME_OPTIONS = [
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
] as const satisfies readonly { value: ThemeMode; label: string }[];
