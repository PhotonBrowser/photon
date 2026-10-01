import { TooltipProvider } from "@gpuix/react";
import { useState } from "react";
import { getStartupUrl } from "../config";
import { delay, MotionPreferenceProvider } from "../motion";
import { ThemeProvider, useThemeMode } from "../theme";
import { BrowserChrome } from "./BrowserChrome";
import { BrowserContent } from "./BrowserContent";

/**
 * The browser window.
 *
 * This is the composition root and the only place the providers live: appearance
 * and motion preferences come from here, and every control below reads them from
 * context instead of taking a theme prop. Browser state is the window's business
 * and never reaches the generic components.
 */
export function BrowserWindow() {
  const theme = useThemeMode();
  const startupUrl = getStartupUrl();
  /** The page on screen. Only a submitted address changes it. */
  const [page, setPage] = useState(startupUrl);
  /** What the field shows. Separate from the page so typing never navigates. */
  const [address, setAddress] = useState(startupUrl);

  return (
    <MotionPreferenceProvider>
      <ThemeProvider
        mode={theme.mode}
        appearance={theme.appearance}
        setMode={theme.setMode}
      >
        <TooltipProvider
          delayDuration={delay.tooltip}
          skipDelayDuration={delay.tooltipSkip}
        >
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              width: "100%",
              height: "100%",
            }}
          >
            <BrowserChrome
              address={address}
              onAddressChange={setAddress}
              onAddressSubmit={setPage}
              appearance={theme.appearance}
              themeMode={theme.mode}
              onThemeModeChange={theme.setMode}
            />
            <BrowserContent url={page} />
          </div>
        </TooltipProvider>
      </ThemeProvider>
    </MotionPreferenceProvider>
  );
}
