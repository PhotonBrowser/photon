import type { MotionTransition } from "@gpuix/react";
import { useReducedMotion } from "./preference";
import type { MotionSpec } from "./tokens";
import { transitionFor } from "./tokens";

export { MotionPreferenceProvider, useReducedMotion } from "./preference";
export type {
  MotionDuration,
  MotionEasing,
  MotionSpec,
} from "./tokens";
export {
  delay,
  duration,
  easing,
  overlayCollisionPadding,
  overlayOffset,
  transitionFor,
} from "./tokens";

/**
 * The transition for a `motion.div`, honouring reduced motion.
 *
 * Pass `undefined` when reduced motion applies and the element should mount at
 * its target instead of animating into it.
 */
export function useTransition(spec?: MotionSpec): MotionTransition | undefined {
  const reduced = useReducedMotion();
  if (!spec) return undefined;
  return transitionFor(spec, reduced);
}
