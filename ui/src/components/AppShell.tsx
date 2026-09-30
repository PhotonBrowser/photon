import { Titlebar } from "./Titlebar";
import { Workspace } from "./Workspace";
import type { Theme, ThemeMode } from "../theme";

interface AppShellProps {
  theme: Theme;
  themeMode: ThemeMode;
  toggleTheme: () => void;
}

export function AppShell({ theme, themeMode, toggleTheme }: AppShellProps) {
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
        onToggleTheme={toggleTheme}
      />
      <Workspace theme={theme} />
    </div>
  );
}
