import { useEffect, useState } from "react";
import { themes, type ThemeAppearance, type ThemeMode } from "../theme";

export function useThemeMode() {
  const [themeMode, setThemeMode] = useState<ThemeMode>("system");
  const [systemAppearance, setSystemAppearance] =
    useState<ThemeAppearance>("dark");

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    ) {
      return;
    }

    const preference = window.matchMedia("(prefers-color-scheme: light)");
    const updateAppearance = () =>
      setSystemAppearance(preference.matches ? "light" : "dark");

    updateAppearance();
    preference.addEventListener("change", updateAppearance);
    return () => preference.removeEventListener("change", updateAppearance);
  }, []);

  const appearance = themeMode === "system" ? systemAppearance : themeMode;

  return {
    themeMode,
    appearance,
    theme: themes[appearance],
    cycleTheme: () =>
      setThemeMode((mode) =>
        mode === "system" ? "dark" : mode === "dark" ? "light" : "system",
      ),
  };
}
