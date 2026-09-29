import { useState } from "react";
import { Titlebar } from "./components/Titlebar";
import { themes, type ThemeMode } from "./theme";

export function App() {
  const [themeMode, setThemeMode] = useState<ThemeMode>("dark");
  const theme = themes[themeMode];

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
      }}
    >
      <Titlebar
        theme={theme}
        themeMode={themeMode}
        onToggleTheme={() =>
          setThemeMode(themeMode === "dark" ? "light" : "dark")
        }
      />
      <div style={{ flexGrow: 1, backgroundColor: theme.color.window }} />
    </div>
  );
}
