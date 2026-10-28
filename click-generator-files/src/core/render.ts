import { helpers } from "./helpers";

export interface Pattern {
  bpm: number;
  steps: number[];
}

export interface RenderResult {
  samples: Float32Array;
  sampleRate: number;
  durationSec: number;
  beatCount: number;
}

export type ClickFn = (
  intensity: number,
  step: number,
  count: number,
  sampleRate: number,
) => Float32Array;

export class CompileError extends Error {}

export function compileClick(code: string): ClickFn {
  try {
    const factory = new Function(
      ...Object.keys(helpers),
      `${code}\nreturn click;`,
    ) as (...args: unknown[]) => unknown;
    const fn = factory(...Object.values(helpers));
    if (typeof fn !== "function") {
      throw new Error("code must define a function named `click`");
    }
    return fn as ClickFn;
  } catch (err) {
    if (err instanceof Error) throw new CompileError(err.message);
    throw new CompileError(String(err));
  }
}

export function renderPattern(
  code: string,
  pattern: Pattern,
  sampleRate: number,
): RenderResult {
  const clickFn = compileClick(code);
  const stepCount = pattern.steps.length;
  const beatSec = 60 / pattern.bpm;
  const beatSamples = Math.round(beatSec * sampleRate);

  const clicks: { start: number; data: Float32Array }[] = [];
  let maxEnd = 0;

  for (let i = 0; i < stepCount; i++) {
    const intensity = Math.max(0, Math.min(1, pattern.steps[i] ?? 0));
    if (intensity <= 0) continue;

    const data = clickFn(intensity, i, stepCount, sampleRate);
    if (!(data instanceof Float32Array) || data.length === 0) {
      throw new CompileError(
        `click() at step ${i + 1} must return a non-empty Float32Array`,
      );
    }
    const start = i * beatSamples;
    const end = start + data.length;
    if (end > maxEnd) maxEnd = end;
    clicks.push({ start, data });
  }

  if (maxEnd === 0) maxEnd = stepCount * beatSamples;

  const samples = new Float32Array(maxEnd);
  for (const { start, data } of clicks) {
    for (let i = 0; i < data.length; i++) {
      const v = samples[start + i] + data[i];
      samples[start + i] = v > 1 ? 1 : v < -1 ? -1 : v;
    }
  }

  return {
    samples,
    sampleRate,
    durationSec: maxEnd / sampleRate,
    beatCount: stepCount,
  };
}
