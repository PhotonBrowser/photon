import { useState } from "react";
import { themes, type ThemeMode } from "../theme";

export function useThemeMode() {
  const [themeMode, setThemeMode] = useState<ThemeMode>("dark");

  return {
    themeMode,
    theme: themes[themeMode],
    toggleTheme: () => {
      setThemeMode((mode) => (mode === "dark" ? "light" : "dark"));
    },
  };
}
