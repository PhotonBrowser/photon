import moonSvg from "../assets/icons/moon.svg" with { type: "text" };
import sunSvg from "../assets/icons/sun.svg" with { type: "text" };
import { theme } from "../theme";
import type { Theme, ThemeMode } from "../theme";
import { IconButton } from "./Button";

const CONTROL_CLEARANCE =
  theme.layout.titlebarControlInset + theme.layout.titlebarControlSize * 3;

interface TitlebarProps {
  theme: Theme;
  themeMode: ThemeMode;
  onToggleTheme: () => void;
}

function ThemeIcon({
  themeMode,
  color,
}: {
  themeMode: ThemeMode;
  color: string;
}) {
  const source = themeMode === "dark" ? sunSvg : moonSvg;
  return <svg source={source} style={{ width: 16, height: 16, color }} />;
}

export function Titlebar({
  theme: activeTheme,
  themeMode,
  onToggleTheme,
}: TitlebarProps) {
  return (
    <div
      style={{
        display: "flex",
        flexShrink: 0,
        alignItems: "center",
        paddingTop: theme.layout.titlebarVerticalPadding,
        paddingBottom: theme.layout.titlebarVerticalPadding,
        color: activeTheme.color.titlebarText,
        fontSize: theme.typography.titlebar,
        backgroundColor: activeTheme.color.titlebar,
      }}
    >
      <div style={{ width: CONTROL_CLEARANCE, flexShrink: 0 }} />
      <div style={{ flexGrow: 1 }} />
      <IconButton
        label={`Switch to ${themeMode === "dark" ? "light" : "dark"} theme`}
        onClick={onToggleTheme}
        color="transparent"
        pressedColor={activeTheme.color.buttonPressed}
        iconColor={activeTheme.color.icon}
      >
        <ThemeIcon themeMode={themeMode} color={activeTheme.color.icon} />
      </IconButton>
      <div style={{ width: theme.layout.titlebarActionInset }} />
    </div>
  );
}
