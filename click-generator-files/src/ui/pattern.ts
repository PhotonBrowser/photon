import type { Pattern } from "../core/render";

export interface PatternCallbacks {
  onChange: (pattern: Pattern) => void;
}

export class PatternEditor {
  private bpmInput: HTMLInputElement;
  private bpmOut: HTMLOutputElement;
  private countSel: HTMLSelectElement;
  private stepsRow: HTMLDivElement;
  private pattern: Pattern;

  constructor(private cbs: PatternCallbacks) {
    const root = document.getElementById("pattern") as HTMLElement;

    const controls = document.createElement("div");
    controls.className = "pattern-controls";

    const bpmLabel = document.createElement("label");
    bpmLabel.className = "field";
    bpmLabel.appendChild(
      Object.assign(document.createElement("span"), { textContent: "BPM" }),
    );
    this.bpmInput = document.createElement("input");
    this.bpmInput.type = "range";
    this.bpmInput.min = "40";
    this.bpmInput.max = "240";
    this.bpmInput.step = "1";
    this.bpmInput.value = "120";
    this.bpmOut = document.createElement("output");
    this.bpmOut.textContent = "120";
    bpmLabel.append(this.bpmInput, this.bpmOut);

    const countLabel = document.createElement("label");
    countLabel.className = "field";
    countLabel.appendChild(
      Object.assign(document.createElement("span"), { textContent: "Steps" }),
    );
    this.countSel = document.createElement("select");
    for (const n of [4, 8, 16]) {
      const opt = document.createElement("option");
      opt.value = String(n);
      opt.textContent = String(n);
      this.countSel.appendChild(opt);
    }
    countLabel.appendChild(this.countSel);

    controls.append(bpmLabel, countLabel);

    this.stepsRow = document.createElement("div");
    this.stepsRow.className = "steps";

    root.append(controls, this.stepsRow);

    this.pattern = { bpm: 120, steps: [1, 1, 1, 1, 1, 1, 1, 1] };

    this.bpmInput.addEventListener("input", () => {
      this.bpmOut.value = this.bpmInput.value;
      this.emit();
    });
    this.countSel.addEventListener("change", () => {
      this.resizeSteps(Number(this.countSel.value));
      this.rebuild();
      this.emit();
    });
  }

  setPattern(p: Pattern): void {
    this.pattern = { bpm: p.bpm, steps: [...p.steps] };
    this.bpmInput.value = String(p.bpm);
    this.bpmOut.value = String(p.bpm);
    this.countSel.value = String(p.steps.length);
    this.rebuild();
  }

  private resizeSteps(count: number): void {
    const cur = this.pattern.steps;
    if (cur.length === count) return;
    const next: number[] = [];
    for (let i = 0; i < count; i++) next.push(cur[i] ?? 0.5);
    this.pattern.steps = next;
  }

  private rebuild(): void {
    this.stepsRow.replaceChildren();
    this.pattern.steps.forEach((v, i) => {
      const cell = document.createElement("label");
      cell.className = "step-cell";

      const num = Object.assign(document.createElement("span"), {
        textContent: String(i + 1),
        className: "step-num",
      });

      const input = document.createElement("input");
      input.type = "range";
      input.min = "0";
      input.max = "100";
      input.step = "1";
      input.value = String(Math.round(v * 100));

      const out = Object.assign(document.createElement("output"), {
        textContent: String(Math.round(v * 100)),
      });

      input.addEventListener("input", () => {
        this.pattern.steps[i] = Number(input.value) / 100;
        out.textContent = input.value;
        this.emit();
      });

      cell.append(num, input, out);
      this.stepsRow.appendChild(cell);
    });
  }

  private emit(): void {
    this.cbs.onChange({
      bpm: Number(this.bpmInput.value),
      steps: [...this.pattern.steps],
    });
  }
}
