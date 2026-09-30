import { theme } from "./theme";

export const TRAFFIC_LIGHT_X = theme.layout.titlebarControlInset;
export const TITLEBAR_HEIGHT =
  theme.layout.titlebarActionSize + theme.layout.titlebarVerticalPadding * 2;
export const TRAFFIC_LIGHT_Y =
  (TITLEBAR_HEIGHT - theme.layout.titlebarControlSize) / 2;
