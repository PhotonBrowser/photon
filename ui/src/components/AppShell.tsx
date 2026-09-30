import { Titlebar } from "./Titlebar";
import { Workspace } from "./Workspace";
import type { Theme, ThemeAppearance, ThemeMode } from "../theme";

interface AppShellProps {
  theme: Theme;
  themeMode: ThemeMode;
  appearance: ThemeAppearance;
  cycleTheme: () => void;
}

export function AppShell({
  theme,
  themeMode,
  appearance,
  cycleTheme,
}: AppShellProps) {
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
        appearance={appearance}
        onCycleTheme={cycleTheme}
      />
      <Workspace theme={theme} />
    </div>
  );
}
