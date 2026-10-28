export interface TransportCallbacks {
  onPlay: () => void;
  onStop: () => void;
}

export class Transport {
  private playBtn: HTMLButtonElement;
  private stopBtn: HTMLButtonElement;

  constructor(private cbs: TransportCallbacks) {
    this.playBtn = document.getElementById("play") as HTMLButtonElement;
    this.stopBtn = document.getElementById("stop") as HTMLButtonElement;
    this.playBtn.addEventListener("click", () => this.cbs.onPlay());
    this.stopBtn.addEventListener("click", () => this.cbs.onStop());
  }

  setPlaying(playing: boolean): void {
    this.playBtn.disabled = playing;
    this.stopBtn.disabled = !playing;
  }
}
