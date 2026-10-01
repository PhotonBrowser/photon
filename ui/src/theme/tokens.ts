/**
 * Photon's visual scale.
 *
 * Every number a component needs to describe size comes from here, so a design
 * change lands in one file instead of in scattered inline literals. Values are
 * pixels unless noted; GPUIX maps plain numbers straight onto GPUI.
 *
 * Colours live in `./colors.ts` and animation timing in `../motion`.
 */

/** Spacing steps. Use the named step, never a one-off pixel value. */
export const space = {
  none: 0,
  xxs: 2,
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 20,
  xxl: 24,
} as const;

/** Corner radii, keyed by role rather than by size. */
export const radii = {
  none: 0,
  xs: 2,
  sm: 4,
  /** Page and control surfaces. */
  md: 6,
  /** Menu rows. */
  lg: 8,
  /** Menus, tooltips, tab strips. */
  xl: 12,
  pill: 999,
} as const;

/** Stroke widths. A hairline is the only border weight most UI needs. */
export const borders = {
  hairline: 1,
  thick: 2,
} as const;

export const fontSize = {
  caption: 11,
  small: 12,
  body: 13,
  title: 14,
  large: 16,
} as const;

export const fontWeight = {
  regular: 400,
  medium: 500,
  semibold: 600,
} as const;

export const lineHeight = {
  tight: 1.2,
  normal: 1.4,
} as const;

/** Intrinsic sizes of the controls Photon composes from. */
export const sizes = {
  /** Smallest decorative icon: control glyph, menu row glyph, tab favicon. */
  iconSm: 16,
  iconMd: 18,
  /** Large glyph, e.g. a hero action. */
  iconLg: 20,
  controlSm: 26,
  controlMd: 30,
  controlLg: 34,
  addressBarMinHeight: 30,
  tabHeight: 32,
  tabMinWidth: 120,
  tabMaxWidth: 220,
  menuMinWidth: 156,
  menuItemHeight: 30,
  tooltipMaxWidth: 260,
} as const;

/**
 * Window chrome geometry. `TITLEBAR_*` in `../windowLayout` derives from these,
 * so native traffic-light placement and the React titlebar can never disagree.
 */
export const layout = {
  /** Left inset of the native traffic lights. */
  trafficLightInset: 12,
  /** Diameter of one native traffic light, used to reserve space for three. */
  trafficLightSize: 14,
  /** Horizontal spacing between the native traffic lights. */
  trafficLightGap: 9,
  /** Vertical padding of the window chrome. */
  chromePaddingY: space.xs,
  /** Inset after the appearance control at the trailing edge. */
  chromeActionInset: 6,
  titlebarHeight: sizes.controlSm + space.xs * 2,
} as const;

/** Width reserved on the leading edge of the titlebar for native window controls. */
export const titlebarControlClearance =
  layout.trafficLightInset +
  layout.trafficLightSize * 3 +
  layout.trafficLightGap * 2;
