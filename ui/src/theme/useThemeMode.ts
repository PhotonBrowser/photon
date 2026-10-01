import { useEffect, useState } from "react";
import type { ThemeAppearance } from "./colors";
import { themes } from "./colors";
import type { ThemeMode } from "./ThemeProvider";

const LIGHT_APPEARANCE_QUERY = "(prefers-color-scheme: light)";

function mediaQuery(query: string): MediaQueryList | null {
  const host = globalThis as { matchMedia?: (query: string) => MediaQueryList };
  return host.matchMedia?.(query) ?? null;
}

function useSystemAppearance(): ThemeAppearance {
  const [appearance, setAppearance] = useState<ThemeAppearance>("dark");

  useEffect(() => {
    const preference = mediaQuery(LIGHT_APPEARANCE_QUERY);
    if (!preference) return;
    const update = () => setAppearance(preference.matches ? "light" : "dark");
    update();
    preference.addEventListener("change", update);
    return () => preference.removeEventListener("change", update);
  }, []);

  return appearance;
}

/** Appearance state for the whole shell, resolved from a mode and the system. */
export function useThemeMode() {
  const [mode, setMode] = useState<ThemeMode>("system");
  const systemAppearance = useSystemAppearance();
  const appearance = mode === "system" ? systemAppearance : mode;

  return {
    mode,
    appearance,
    colors: themes[appearance],
    setMode,
  };
}
