import type { Props, StyleDesc } from "@gpuix/react";
import {
  AnimatePresence,
  DismissableLayer,
  Button as GpuixButton,
  motion,
} from "@gpuix/react";
import { FloatingLayer } from "@gpuix/react/floating";
import type { ReactNode } from "react";
import { createContext, useContext } from "react";
import {
  overlayCollisionPadding,
  overlayOffset,
  useTransition,
} from "../motion";
import { useThemeColors } from "../theme";
import { radii, sizes, space } from "../theme/tokens";
import { Surface } from "./Surface";

interface PopoverContextValue {
  open: boolean;
  toggle: () => void;
  close: () => void;
}

const PopoverContext = createContext<PopoverContextValue | null>(null);

function usePopoverContext(component: string): PopoverContextValue {
  const context = useContext(PopoverContext);
  if (!context) throw new Error(`${component} must be used inside Popover`);
  return context;
}

export interface PopoverProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children?: ReactNode;
}

/**
 * An overlay anchored to a control, holding any content.
 *
 * Where `Menu` offers a list of choices, `Popover` holds a panel: a settings
 * group, a page preview, a small form. Both go through GPUIX's anchored layer,
 * so neither can become a positioned card that a scrolling list paints over.
 */
export function Popover({ open, onOpenChange, children }: PopoverProps) {
  const value: PopoverContextValue = {
    open,
    toggle: () => onOpenChange(!open),
    close: () => onOpenChange(false),
  };
  return (
    <PopoverContext.Provider value={value}>{children}</PopoverContext.Provider>
  );
}

export interface PopoverTriggerProps extends Omit<Props, "style"> {
  /**
   * Merge the popover's behaviour into the child instead of wrapping it, so a
   * trigger stays one control with one focus stop and one hitbox.
   */
  asChild?: boolean;
}

/**
 * The control that opens the popover.
 *
 * GPUIX's Button provides pointer and keyboard activation while merging the
 * behavior into the child, so the trigger stays one control and one hitbox.
 */
export function PopoverTrigger({
  asChild = true,
  children,
  ...props
}: PopoverTriggerProps) {
  const { open, toggle } = usePopoverContext("PopoverTrigger");
  return (
    <GpuixButton
      {...props}
      asChild={asChild}
      aria-expanded={open}
      onClick={toggle}
    >
      {children}
    </GpuixButton>
  );
}

export interface PopoverContentProps extends Omit<Props, "style"> {
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  sideOffset?: number;
  /** Merge over the overlay surface tokens, for width and padding. */
  style?: StyleDesc;
  children?: ReactNode;
}

/**
 * The overlay, mounted only while the popover is open.
 *
 * `AnimatePresence` keeps the anchored layer mounted for the length of the exit
 * tween, so the panel fades instead of disappearing on the frame the state flips.
 * The tween itself is native: Rust interpolates the opacity and asks GPUI for
 * the frames, so a closing popover costs no React renders.
 */
export function PopoverContent(props: PopoverContentProps) {
  const { open } = usePopoverContext("PopoverContent");
  return (
    <AnimatePresence initial={false}>
      {open && <PopoverSurface key="popover-surface" {...props} />}
    </AnimatePresence>
  );
}

function PopoverSurface({
  align = "start",
  side = "bottom",
  sideOffset = overlayOffset.default,
  style,
  children,
  ...props
}: PopoverContentProps) {
  const colors = useThemeColors();
  const { close } = usePopoverContext("PopoverContent");
  const transition = useTransition({ duration: "quick" });

  return (
    <DismissableLayer onEscapeKeyDown={close}>
      <FloatingLayer
        {...props}
        side={side}
        align={align}
        sideOffset={sideOffset}
        collisionPadding={overlayCollisionPadding}
        onMouseDownOutside={close}
        // Keep GPUIX's anchored fallback fill clipped to the inner surface.
        style={{ borderRadius: radii.xl }}
      >
        <Surface
          variant="menu"
          style={{
            minWidth: sizes.menuMinWidth,
            padding: space.sm,
            gap: space.xs,
            color: colors.text,
            ...style,
          }}
        >
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={transition}
            style={{ display: "flex", flexDirection: "column", gap: space.xs }}
          >
            {children}
          </motion.div>
        </Surface>
      </FloatingLayer>
    </DismissableLayer>
  );
}
