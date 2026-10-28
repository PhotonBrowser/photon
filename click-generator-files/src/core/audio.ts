let ctx: AudioContext | null = null;
let currentSource: AudioBufferSourceNode | null = null;

export function getAudioContext(): AudioContext {
  if (!ctx) ctx = new AudioContext();
  return ctx;
}

export async function ensureRunning(): Promise<void> {
  const c = getAudioContext();
  if (c.state === "suspended") await c.resume();
}

export function playBuffer(
  samples: Float32Array,
  sampleRate: number,
  onEnded?: () => void,
): void {
  stopPlayback();
  const c = getAudioContext();
  const buffer = c.createBuffer(1, samples.length, sampleRate);
  buffer.copyToChannel(samples, 0);

  const src = c.createBufferSource();
  src.buffer = buffer;
  src.connect(c.destination);
  src.onended = () => {
    if (currentSource === src) currentSource = null;
    onEnded?.();
  };
  currentSource = src;
  src.start();
}

export function stopPlayback(): void {
  if (currentSource) {
    try {
      currentSource.stop();
    } catch {
      /* already stopped */
    }
    currentSource = null;
  }
}
