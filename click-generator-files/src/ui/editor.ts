export interface EditorCallbacks {
  onChange: (code: string) => void;
}

export class Editor {
  private ta: HTMLTextAreaElement;
  private errorEl: HTMLElement;
  private timer: number | undefined;

  constructor(private cbs: EditorCallbacks) {
    this.ta = document.getElementById("code") as HTMLTextAreaElement;
    this.errorEl = document.getElementById("error") as HTMLElement;

    this.ta.addEventListener("input", () => {
      window.clearTimeout(this.timer);
      this.timer = window.setTimeout(() => this.cbs.onChange(this.ta.value), 250);
    });
  }

  setCode(code: string): void {
    this.ta.value = code;
  }

  getCode(): string {
    return this.ta.value;
  }

  showError(msg: string | null): void {
    if (msg) {
      this.errorEl.textContent = msg;
      this.errorEl.hidden = false;
    } else {
      this.errorEl.hidden = true;
      this.errorEl.textContent = "";
    }
  }
}
