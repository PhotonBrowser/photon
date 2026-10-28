import type { RenderResult } from "../core/render";
import { encodeWav } from "../core/wav";

export interface ExportCallbacks {
  onGenerate: () => RenderResult;
  onFilename: () => string;
  onRateChange: () => void;
}

export class Exporter {
  private rateSel: HTMLSelectElement;
  private genBtn: HTMLButtonElement;
  private dlLink: HTMLAnchorElement;
  private infoEl: HTMLElement;
  private url: string | null = null;

  constructor(private cbs: ExportCallbacks) {
    this.rateSel = document.getElementById("sample-rate") as HTMLSelectElement;
    this.genBtn = document.getElementById("generate") as HTMLButtonElement;
    this.dlLink = document.getElementById("download") as HTMLAnchorElement;
    this.infoEl = document.getElementById("export-info") as HTMLElement;

    this.genBtn.addEventListener("click", () => this.generate());
    this.rateSel.addEventListener("change", () => this.cbs.onRateChange());
  }

  get sampleRate(): number {
    return Number(this.rateSel.value);
  }

  private generate(): void {
    const result = this.cbs.onGenerate();
    const blob = encodeWav(result.samples, result.sampleRate);
    if (this.url) URL.revokeObjectURL(this.url);
    this.url = URL.createObjectURL(blob);
    this.dlLink.href = this.url;
    this.dlLink.download = this.cbs.onFilename();
    this.dlLink.hidden = false;
    this.infoEl.textContent = `${result.samples.length} samples · ${result.durationSec.toFixed(3)} s · ${(blob.size / 1024).toFixed(1)} KB`;
  }
}
