import { AppShell } from "./components/AppShell";
import { useThemeMode } from "./hooks/useThemeMode";

export function App() {
  const themeState = useThemeMode();
  return <AppShell {...themeState} />;
}
