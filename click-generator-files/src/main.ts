import "./style.css";
import { presets, type Preset } from "./presets";
import { renderPattern, CompileError, type Pattern } from "./core/render";
import { ensureRunning, playBuffer, stopPlayback } from "./core/audio";
import { Editor } from "./ui/editor";
import { PatternEditor } from "./ui/pattern";
import { Transport } from "./ui/transport";
import { Exporter } from "./ui/export";
import { Waveform } from "./ui/waveform";

let currentCode = presets[0].code;
let currentPattern: Pattern = { ...presets[0].pattern, steps: [...presets[0].pattern.steps] };

const editor = new Editor({
  onChange: (code) => {
    currentCode = code;
    rerender();
  },
});
const patternEditor = new PatternEditor({
  onChange: (pattern) => {
    currentPattern = pattern;
    rerender();
  },
});
const waveform = new Waveform();
const exporter = new Exporter({
  onGenerate: () => renderForExport(),
  onFilename: () => buildFilename(),
  onRateChange: () => rerender(),
});
const transport = new Transport({
  onPlay: () => void handlePlay(),
  onStop: () => {
    stopPlayback();
    transport.setPlaying(false);
  },
});

function loadPreset(p: Preset): void {
  editor.setCode(p.code);
  currentCode = p.code;
  currentPattern = { bpm: p.pattern.bpm, steps: [...p.pattern.steps] };
  patternEditor.setPattern(currentPattern);
  rerender();
}

function renderForExport(): ReturnType<typeof renderPattern> {
  const result = renderPattern(currentCode, currentPattern, exporter.sampleRate);
  waveform.draw(result.samples);
  editor.showError(null);
  return result;
}

function rerender(): void {
  try {
    const result = renderPattern(currentCode, currentPattern, exporter.sampleRate);
    waveform.draw(result.samples);
    editor.showError(null);
  } catch (err) {
    editor.showError(
      err instanceof CompileError ? err.message : String(err),
    );
    waveform.clear();
  }
}

async function handlePlay(): Promise<void> {
  try {
    await ensureRunning();
    const result = renderForExport();
    playBuffer(result.samples, result.sampleRate, () => transport.setPlaying(false));
    transport.setPlaying(true);
  } catch (err) {
    editor.showError(String(err));
  }
}

function buildFilename(): string {
  const active = presets.find((p) => p.code === currentCode);
  return `click-${active?.id ?? "custom"}-${Math.round(currentPattern.bpm)}bpm.wav`;
}

const presetBar = document.getElementById("presets") as HTMLElement;
for (const p of presets) {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "preset-btn";
  btn.textContent = p.name;
  btn.title = p.description;
  btn.addEventListener("click", () => loadPreset(p));
  presetBar.appendChild(btn);
}

loadPreset(presets[0]);
transport.setPlaying(false);
