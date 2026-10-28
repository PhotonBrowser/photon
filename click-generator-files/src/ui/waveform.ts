export class Waveform {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;

  constructor() {
    this.canvas = document.getElementById("waveform") as HTMLCanvasElement;
    this.ctx = this.canvas.getContext("2d")!;
    this.resize();
    window.addEventListener("resize", () => this.resize());
  }

  private resize(): void {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    this.canvas.width = Math.max(1, Math.floor(rect.width * dpr));
    this.canvas.height = Math.max(1, Math.floor(rect.height * dpr));
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  draw(samples: Float32Array): void {
    const w = this.canvas.getBoundingClientRect().width;
    const h = this.canvas.getBoundingClientRect().height;
    const ctx = this.ctx;
    ctx.clearRect(0, 0, w, h);

    if (samples.length === 0) return;

    const cols = Math.max(1, Math.floor(w));
    const per = samples.length / cols;
    const mid = h / 2;

    ctx.strokeStyle = "#4fd1c5";
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = 0; x < cols; x++) {
      let min = 1;
      let max = -1;
      const start = Math.floor(x * per);
      const end = Math.min(samples.length, Math.ceil((x + 1) * per));
      for (let i = start; i < end; i++) {
        const v = samples[i];
        if (v < min) min = v;
        if (v > max) max = v;
      }
      const y1 = mid - max * mid * 0.9;
      const y2 = mid - min * mid * 0.9;
      ctx.moveTo(x + 0.5, y1);
      ctx.lineTo(x + 0.5, y2);
    }
    ctx.stroke();
  }

  clear(): void {
    const w = this.canvas.getBoundingClientRect().width;
    const h = this.canvas.getBoundingClientRect().height;
    this.ctx.clearRect(0, 0, w, h);
  }
}
