import type { Pattern } from "./core/render";

export interface Preset {
  id: string;
  name: string;
  description: string;
  code: string;
  pattern: Pattern;
}

export const presets: Preset[] = [
  {
    id: "classic-haptic",
    name: "Classic Haptic",
    description: "Faithful to the web-haptics debug click: white noise with a fast decay through a bandpass filter.",
    code: `function click(intensity, step, count, sampleRate) {
  const duration = Math.floor(sampleRate * 0.004);
  const out = new Float32Array(duration);
  for (let i = 0; i < out.length; i++) {
    out[i] = noise() * expDecay(i, 25);
  }
  return bandpass(out, 2000 + intensity * 2000, 8, sampleRate);
}`,
    pattern: { bpm: 140, steps: [1, 0.75, 0.5, 0.75, 1, 0.6, 0.4, 0.6] },
  },
  {
    id: "soft-tap",
    name: "Soft Tap",
    description: "Mellow finger tap — longer, rounded envelope through a lower, wide bandpass.",
    code: `function click(intensity, step, count, sampleRate) {
  const duration = Math.floor(sampleRate * 0.03);
  const out = new Float32Array(duration);
  for (let i = 0; i < out.length; i++) {
    out[i] = noise() * expDecay(i, 40) * (1 - i / out.length);
  }
  return bandpass(out, 600 + intensity * 400, 1.2, sampleRate);
}`,
    pattern: { bpm: 100, steps: [1, 0.7, 1, 0.7, 0.5, 0.7, 1, 0.7] },
  },
  {
    id: "sharp-snap",
    name: "Sharp Snap",
    description: "Tiny, piercing transient — near-instant decay, high frequency, tight Q.",
    code: `function click(intensity, step, count, sampleRate) {
  const duration = Math.floor(sampleRate * 0.002);
  const out = new Float32Array(duration);
  for (let i = 0; i < out.length; i++) {
    out[i] = noise() * expDecay(i, 8);
  }
  return bandpass(out, 3000 + intensity * 3000, 10, sampleRate);
}`,
    pattern: { bpm: 160, steps: [1, 0.3, 0.8, 0.3, 1, 0.3, 0.8, 0.3] },
  },
  {
    id: "mechanical",
    name: "Mechanical",
    description: "Two-stage key press — a hard attack that settles into a quieter body.",
    code: `function click(intensity, step, count, sampleRate) {
  const duration = Math.floor(sampleRate * 0.015);
  const out = new Float32Array(duration);
  for (let i = 0; i < out.length; i++) {
    const attack = i < out.length * 0.2 ? 1 : 0.4;
    out[i] = noise() * expDecay(i, 10) * attack;
  }
  return bandpass(out, 1200 + intensity * 1800, 2, sampleRate);
}`,
    pattern: { bpm: 120, steps: [1, 0.8, 0.6, 0.8, 1, 1, 0.6, 0.8] },
  },
  {
    id: "sub-thump",
    name: "Sub Thump",
    description: "Low sine burst with a slow decay — a deep, tactile 'bump'.",
    code: `function click(intensity, step, count, sampleRate) {
  const duration = Math.floor(sampleRate * 0.08);
  const out = new Float32Array(duration);
  for (let i = 0; i < out.length; i++) {
    const t = i / sampleRate;
    out[i] = sine(120 + intensity * 60, t) * expDecay(i, 800) * intensity;
  }
  return bandpass(out, 100 + intensity * 200, 1, sampleRate);
}`,
    pattern: { bpm: 110, steps: [1, 0.4, 0.9, 0.4, 0.7, 0.4, 1, 0.4] },
  },
];
