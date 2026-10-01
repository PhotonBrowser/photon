/**
 * Photon's motion tokens.
 *
 * GPUIX interpolates a `motion.div` target natively: React sends the target
 * once and Rust drives the frames, so nothing here animates from JavaScript.
 * Durations are named by intent, never inlined, so the whole app changes
 * tempo in one edit.
 */

import type { MotionEase, MotionTransition } from "@gpuix/react";

/**
 * Durations in seconds, matching GPUIX's `MotionTransition`.
 *
 * `instant` is the reduced-motion value: a zero-length transition still runs
 * the native path, it just does not move.
 */
export const duration = {
  instant: 0,
  /** Hover and press feedback. */
  fast: 0.1,
  /** A control appearing or a selection moving. */
  quick: 0.15,
  /** An overlay opening. */
  normal: 0.2,
  /** A large surface, such as a panel, expanding. */
  slow: 0.28,
} as const;

/** Named easing curves. GPUIX accepts a keyword or a cubic bezier tuple. */
export const easing = {
  linear: "linear",
  /** The default for anything moving in. */
  standard: "easeOut",
  /** Accelerating away, for anything leaving. */
  accelerate: "easeIn",
  /** For geometry that moves on screen, so both ends feel weighted. */
  emphasized: [0.16, 1, 0.3, 1] as MotionEase,
  exit: [0.4, 0, 1, 1] as MotionEase,
} as const;

export type MotionDuration = keyof typeof duration;
export type MotionEasing = keyof typeof easing;

/** A transition described by intent, resolved through the tables above. */
export interface MotionSpec {
  duration: MotionDuration;
  easing?: MotionEasing;
  delay?: MotionDuration;
}

/** Delays accepted by GPUIX timer based controls, in milliseconds. */
export const delay = {
  tooltip: 500,
  /** After a tooltip closes, the next one skips the wait. */
  tooltipSkip: 300,
} as const;

/** Distance an overlay floats from its anchor. */
export const overlayOffset = {
  close: 4,
  default: 8,
} as const;

/** Distance an overlay keeps from the window edge when it has to flip. */
export const overlayCollisionPadding = 8;

/**
 * Resolve a spec into a GPUIX transition. `reduced` collapses it to
 * {@link duration.instant}, which is the reduced-motion path for anything GPUIX
 * animates.
 */
export function transitionFor(
  spec: MotionSpec,
  reduced = false,
): MotionTransition {
  if (reduced) return { duration: duration.instant };
  return {
    duration: duration[spec.duration],
    delay: spec.delay === undefined ? undefined : duration[spec.delay],
    ease: spec.easing === undefined ? easing.standard : easing[spec.easing],
  };
}
