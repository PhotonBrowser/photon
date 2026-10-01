import type { ReactNode } from "react";
import checkSvg from "../assets/icons/check.svg" with { type: "text" };
import monitorSvg from "../assets/icons/monitor.svg" with { type: "text" };
import moonSvg from "../assets/icons/moon.svg" with { type: "text" };
import sunSvg from "../assets/icons/sun.svg" with { type: "text" };
import { AddressBar } from "../components/AddressBar";
import { IconButton } from "../components/Button";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "../components/Menu";
import { Toolbar } from "../components/Toolbar";
import type { ThemeAppearance, ThemeMode } from "../theme";
import { THEME_OPTIONS } from "../theme";
import { layout, titlebarControlClearance } from "../theme/tokens";

const APPEARANCE_ICONS: Record<ThemeAppearance, string> = {
  dark: sunSvg,
  light: moonSvg,
};

const MODE_ICONS: Record<ThemeMode, string> = {
  system: monitorSvg,
  dark: moonSvg,
  light: sunSvg,
};

/**
 * The window chrome.
 *
 * It is the address field plus the appearance control and the window drag
 * regions, and nothing else. The field only hands typed text upwards: deciding
 * whether that text is an address or a query belongs to Rust
 * (`crates/photon-omnibox`), so no rule about it lives in this file. Appearance
 * lives in `theme`, and the controls come from `components`. The leading
 * clearance is reserved for the native window buttons, whose position is
 * derived from the same tokens.
 */
export interface BrowserChromeProps {
  /**
   * What the address field shows: the text being typed, or the address that was
   * last submitted. The field keeps the typed text, because the text is what the
   * person can correct.
   */
  address: string;
  /** Every keystroke, so the draft stays in state. */
  onAddressChange?: (value: string) => void;
  /** Enter in the field. Rust decides whether that is an address or a query. */
  onAddressSubmit?: (value: string) => void;
  appearance: ThemeAppearance;
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
}

export function BrowserChrome({
  address,
  onAddressChange,
  onAddressSubmit,
  appearance,
  themeMode,
  onThemeModeChange,
}: BrowserChromeProps) {
  return (
    <Toolbar variant="chrome">
      <photon-titlebar-drag-region
        style={{
          width: titlebarControlClearance,
          height: "100%",
          flexShrink: 0,
        }}
      />
      <AddressBar
        value={address}
        placeholder="Search or enter address"
        onValueChange={onAddressChange}
        onSubmit={onAddressSubmit}
        testId="address-bar"
        // Fill the titlebar space between the native controls and appearance
        // picker so the address field remains comfortably usable at any width.
        style={{ flexGrow: 1 }}
      />
      <ThemeMenu
        appearance={appearance}
        themeMode={themeMode}
        onThemeModeChange={onThemeModeChange}
      />
      <photon-titlebar-drag-region
        style={{
          width: layout.chromeActionInset,
          height: "100%",
          flexShrink: 0,
        }}
      />
    </Toolbar>
  );
}

interface ThemeMenuProps {
  appearance: ThemeAppearance;
  themeMode: ThemeMode;
  onThemeModeChange: (mode: ThemeMode) => void;
}

/** Appearance picker: the chrome's other control, and a Menu example. */
function ThemeMenu({
  appearance,
  themeMode,
  onThemeModeChange,
}: ThemeMenuProps) {
  const label = `Appearance: ${themeMode}`;
  return (
    <Menu
      value={themeMode}
      onValueChange={(value) => onThemeModeChange(value as ThemeMode)}
    >
      <MenuTrigger>
        <IconButton
          variant="ghost"
          size="sm"
          label={label}
          icon={
            <svg
              source={APPEARANCE_ICONS[appearance]}
              style={{ width: "100%", height: "100%" }}
            />
          }
        />
      </MenuTrigger>
      <MenuContent align="end" side="bottom">
        {THEME_OPTIONS.map((option) => (
          <MenuItem
            key={option.value}
            value={option.value}
            icon={
              <svg
                source={MODE_ICONS[option.value]}
                style={{ width: "100%", height: "100%" }}
              />
            }
            selectedMark={checkMark}
          >
            {option.label}
          </MenuItem>
        ))}
      </MenuContent>
    </Menu>
  );
}

const checkMark: ReactNode = (
  <svg source={checkSvg} style={{ width: "100%", height: "100%" }} />
);
