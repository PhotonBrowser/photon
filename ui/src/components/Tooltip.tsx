import {
  motion,
  TooltipContent,
  Tooltip as TooltipRoot,
  TooltipTrigger,
} from "@gpuix/react";
import type { ReactNode } from "react";
import {
  overlayCollisionPadding,
  overlayOffset,
  useTransition,
} from "../motion";
import { useThemeColors } from "../theme";
import { fontSize, radii, sizes } from "../theme/tokens";
import { Surface } from "./Surface";

export type TooltipSide = "top" | "right" | "bottom" | "left";

export interface TooltipProps {
  /** The hint. Keep it to the control's name or its one shortcut. */
  label: string;
  side?: TooltipSide;
  /** Distance from the control. */
  sideOffset?: number;
  /** Drive the tooltip from the outside, e.g. to explain a state. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  /** Exactly one control: it becomes the trigger and keeps its own props. */
  children: ReactNode;
}

/**
 * A hint attached to a control.
 *
 * GPUIX owns the anchoring, the hover and focus triggers and the delay; this
 * owns the surface tokens and the entrance. The fade is a native `motion.div`,
 * interpolated by Rust rather than by a React render per frame, and it collapses
 * to zero duration under reduced motion.
 *
 * Closing is not animated. GPUIX unmounts tooltip content the moment the close
 * delay expires, and holding it mounted for an exit tween would leave the hint
 * hanging behind the pointer.
 */
export function Tooltip({
  label,
  side = "top",
  sideOffset = overlayOffset.default,
  open,
  onOpenChange,
  children,
}: TooltipProps) {
  const colors = useThemeColors();
  const transition = useTransition({ duration: "quick" });

  return (
    <TooltipRoot open={open} onOpenChange={onOpenChange}>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent
        side={side}
        sideOffset={sideOffset}
        collisionPadding={overlayCollisionPadding}
        // FloatingLayer paints a backing surface beneath its child. Give that
        // backing the same radius as our tooltip surface to avoid square corners.
        style={{ borderRadius: radii.md, pointerEvents: "none" }}
      >
        <Surface
          variant="tooltip"
          style={{
            maxWidth: sizes.tooltipMaxWidth,
            color: colors.text,
            // A hint is never a target: it must not eat the click behind it.
            pointerEvents: "none",
          }}
        >
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={transition}
            style={{ fontSize: fontSize.small }}
          >
            {label}
          </motion.div>
        </Surface>
      </TooltipContent>
    </TooltipRoot>
  );
}
