export const helpers = {
  noise(): number {
    return Math.random() * 2 - 1;
  },

  sine(freq: number, t: number): number {
    return Math.sin(2 * Math.PI * freq * t);
  },

  expDecay(i: number, k: number): number {
    return Math.exp(-i / k);
  },

  bandpass(
    data: Float32Array,
    freq: number,
    q: number,
    sampleRate: number,
  ): Float32Array {
    const w0 = (2 * Math.PI * freq) / sampleRate;
    const alpha = Math.sin(w0) / (2 * q);
    const cosw = Math.cos(w0);
    const b0 = alpha;
    const b1 = 0;
    const b2 = -alpha;
    const a0 = 1 + alpha;
    const a1 = -2 * cosw;
    const a2 = 1 - alpha;

    let x1 = 0;
    let x2 = 0;
    let y1 = 0;
    let y2 = 0;

    for (let i = 0; i < data.length; i++) {
      const xn = data[i];
      const yn = (b0 * xn + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
      x2 = x1;
      x1 = xn;
      y2 = y1;
      y1 = yn;
      data[i] = yn;
    }
    return data;
  },
};

export type HelperName = keyof typeof helpers;
