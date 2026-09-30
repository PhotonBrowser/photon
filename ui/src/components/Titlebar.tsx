import moonSvg from "../assets/icons/moon.svg" with { type: "text" };
import sunSvg from "../assets/icons/sun.svg" with { type: "text" };
import { theme } from "../theme";
import type { Theme, ThemeAppearance, ThemeMode } from "../theme";
import { IconButton } from "./Button";

const CONTROL_CLEARANCE =
  theme.layout.titlebarControlInset + theme.layout.titlebarControlSize * 3;

interface TitlebarProps {
  theme: Theme;
  themeMode: ThemeMode;
  appearance: ThemeAppearance;
  onCycleTheme: () => void;
}

function ThemeIcon({
  themeMode,
  color,
}: {
  themeMode: ThemeAppearance;
  color: string;
}) {
  const source = themeMode === "dark" ? sunSvg : moonSvg;
  return <svg source={source} style={{ width: 16, height: 16, color }} />;
}

export function Titlebar({
  theme: activeTheme,
  themeMode,
  appearance,
  onCycleTheme,
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
        backgroundColor: activeTheme.color.window,
      }}
    >
      <photon-titlebar-drag-region
        style={{ width: CONTROL_CLEARANCE, height: "100%", flexShrink: 0 }}
      />
      <photon-titlebar-drag-region style={{ flexGrow: 1, height: "100%" }} />
      <IconButton
        label={`Theme: ${themeMode}. Switch to ${themeMode === "system" ? "dark" : themeMode === "dark" ? "light" : "system"}`}
        onClick={onCycleTheme}
        color="transparent"
        hoverColor={activeTheme.color.buttonHover}
        pressedColor={activeTheme.color.buttonPressed}
        iconColor={activeTheme.color.icon}
      >
        <ThemeIcon themeMode={appearance} color={activeTheme.color.icon} />
      </IconButton>
      <photon-titlebar-drag-region
        style={{ width: theme.layout.titlebarActionInset, height: "100%" }}
      />
    </div>
  );
}
