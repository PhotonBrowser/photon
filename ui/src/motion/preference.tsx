import type { ReactNode } from "react";
import { createContext, useContext, useEffect, useState } from "react";

/**
 * Reduced-motion preference.
 *
 * GPUIX 0.10 exposes no reduced-motion setting, so this reads the CSS media
 * query when the runtime has one and otherwise defers to the explicit `reduced`
 * prop. That prop is the seam for a native value later: wiring one in changes
 * nothing in any component.
 */

const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

interface MotionPreferenceValue {
  reduced: boolean;
}

const MotionPreferenceContext = createContext<MotionPreferenceValue>({
  reduced: false,
});

interface MotionPreferenceProviderProps {
  /** Force reduced motion on or off instead of following the media query. */
  reduced?: boolean;
  children?: ReactNode;
}

export function MotionPreferenceProvider({
  reduced = false,
  children,
}: MotionPreferenceProviderProps) {
  return (
    <MotionPreferenceContext.Provider value={{ reduced }}>
      {children}
    </MotionPreferenceContext.Provider>
  );
}

function reducedMotionQuery(): MediaQueryList | null {
  const host = globalThis as { matchMedia?: (query: string) => MediaQueryList };
  return host.matchMedia?.(REDUCED_MOTION_QUERY) ?? null;
}

/** True when animation should be skipped or collapsed to zero duration. */
export function useReducedMotion(): boolean {
  const { reduced } = useContext(MotionPreferenceContext);
  const [systemReduced, setSystemReduced] = useState(
    () => reducedMotionQuery()?.matches ?? false,
  );

  useEffect(() => {
    const query = reducedMotionQuery();
    if (!query) return;
    const update = () => setSystemReduced(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  return reduced || systemReduced;
}
