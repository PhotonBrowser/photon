#!/usr/bin/env python3
"""Verify that Photon renders a local web page through the Linux Engine path."""

from __future__ import annotations

import http.server
import os
import signal
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from urllib.parse import parse_qs, urlsplit


ROOT = Path(__file__).resolve().parents[1]
PAGE_TITLE = "PHOTON_LINUX_RENDER_READY"
PAGE_COLOR = (36, 162, 90)
MIN_MATCHING_PIXELS = 5_000
RENDER_TIMEOUT_SECONDS = 60

PAGE = b"""<!doctype html>
<meta charset="utf-8">
<title>Photon Linux render smoke</title>
<style>
  html, body, canvas { width: 100%; height: 100%; margin: 0; overflow: hidden; }
  canvas { display: block; }
</style>
<canvas></canvas>
<script>
const canvas = document.querySelector('canvas');
const scale = window.devicePixelRatio || 1;
canvas.width = Math.max(1, Math.floor(window.innerWidth * scale));
canvas.height = Math.max(1, Math.floor(window.innerHeight * scale));
const context = canvas.getContext('2d');
context.fillStyle = '#24a25a';
context.fillRect(0, 0, canvas.width, canvas.height);
context.fillStyle = '#d04462';
context.fillRect(0, 0, canvas.width / 4, canvas.height / 4);
document.title = 'PHOTON_LINUX_RENDER_READY';
window.addEventListener('load', () => requestAnimationFrame(() => {
  const pixel = Array.from(context.getImageData(
    Math.floor(canvas.width / 2), Math.floor(canvas.height / 2), 1, 1
  ).data.slice(0, 3)).join(',');
  fetch(`/__ready?pixel=${pixel}`, { cache: 'no-store' }).catch(() => {});
}));
</script>"""


class SmokeServer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), SmokeHandler)
        self.page_ready = threading.Event()


class SmokeHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        path = urlsplit(self.path)
        if path.path == "/__ready":
            pixel = parse_qs(path.query).get("pixel", [""])[0]
            if pixel != ",".join(map(str, PAGE_COLOR)):
                self.send_error(400, "page canvas did not contain the expected pixel")
                return
            self.server.page_ready.set()  # type: ignore[attr-defined]
            self.send_response(204)
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            return

        if path.path not in ("/", "/index.html"):
            self.send_error(404)
            return

        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(PAGE)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(PAGE)

    def log_message(self, _format: str, *_args: object) -> None:
        pass


def screenshot_contains_page_color(work_dir: Path) -> tuple[int, Path]:
    xwd_path = work_dir / "screen.xwd"
    ppm_path = work_dir / "screen.ppm"
    subprocess.run(
        ["xwd", "-root", "-silent", "-out", str(xwd_path)],
        check=True,
        capture_output=True,
        text=True,
    )
    subprocess.run(
        ["convert", str(xwd_path), "-depth", "8", str(ppm_path)],
        check=True,
        capture_output=True,
        text=True,
    )

    from PIL import Image

    image = Image.open(ppm_path).convert("RGB")
    matches = sum(
        1
        for red, green, blue in image.getdata()
        if abs(red - PAGE_COLOR[0]) <= 18
        and abs(green - PAGE_COLOR[1]) <= 18
        and abs(blue - PAGE_COLOR[2]) <= 18
    )
    screenshot = os.environ.get("PHOTON_RENDER_SCREENSHOT")
    screenshot_path = Path(screenshot) if screenshot else work_dir / "screen.png"
    screenshot_path.parent.mkdir(parents=True, exist_ok=True)
    image.save(screenshot_path)
    return matches, screenshot_path


def stop_photon(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def run_smoke() -> None:
    with tempfile.TemporaryDirectory(prefix="photon-linux-render-") as temporary:
        work_dir = Path(temporary)
        home = work_dir / "home"
        profile = work_dir / "profile"
        runtime = work_dir / "runtime"
        for directory in (home, profile, runtime):
            directory.mkdir(mode=0o700)
        profile.joinpath("config.toml").write_text(
            'restore_session = false\ntab_sleep_mode = "off"\n', encoding="utf-8"
        )

        server = SmokeServer()
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()
        url = f"http://127.0.0.1:{server.server_address[1]}/"
        environment = os.environ.copy()
        environment.update(
            {
                "HOME": str(home),
                "PHOTON_PROFILE_DIR": str(profile),
                "PHOTON_NAVIGATION_PROFILE": "1",
                "PHOTON_VERBOSE": "1",
                "XDG_CONFIG_HOME": str(home / ".config"),
                "XDG_CACHE_HOME": str(home / ".cache"),
                "XDG_DATA_HOME": str(home / ".local/share"),
                "XDG_RUNTIME_DIR": str(runtime),
                "XDG_SESSION_TYPE": "x11",
            }
        )
        environment.pop("WAYLAND_DISPLAY", None)

        log_path = work_dir / "photon.log"
        process: subprocess.Popen[bytes] | None = None
        screenshot_matches: int | None = None
        try:
            with log_path.open("wb") as log_file:
                process = subprocess.Popen(
                    [
                        str(ROOT / "photon"),
                        "run",
                        "--debug",
                        "--url",
                        url,
                        "--shutdown-after-seconds",
                        "120",
                    ],
                    cwd=ROOT,
                    env=environment,
                    stdin=subprocess.DEVNULL,
                    stdout=log_file,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )

                deadline = time.monotonic() + RENDER_TIMEOUT_SECONDS
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        break
                    output = log_path.read_text(encoding="utf-8", errors="replace")
                    if server.page_ready.is_set() and "event=first_page_render" in output:
                        time.sleep(0.5)
                        screenshot_matches, screenshot = screenshot_contains_page_color(work_dir)
                        if screenshot_matches < MIN_MATCHING_PIXELS:
                            raise RuntimeError(
                                f"only {screenshot_matches} pixels matched the page canvas color "
                                f"(need {MIN_MATCHING_PIXELS}); screenshot: {screenshot}"
                            )
                        print(
                            "Linux page render verified: "
                            f"{screenshot_matches} canvas-color pixels; screenshot: {screenshot}"
                        )
                        break
                    time.sleep(0.2)
                else:
                    raise TimeoutError(
                        "timed out waiting for the page canvas beacon and first GPUI page render"
                    )

                stop_photon(process)

            output = log_path.read_text(encoding="utf-8", errors="replace")
            if process.returncode != 0:
                raise RuntimeError(f"photon run exited with {process.returncode}")
            if not server.page_ready.is_set():
                raise RuntimeError("the local page did not report its canvas pixel")
            if "event=first_page_render" not in output:
                raise RuntimeError("Photon did not render an Engine page frame through GPUI")
            if f"title={PAGE_TITLE}" not in output:
                raise RuntimeError("Photon did not receive the page's JavaScript-set title")
            if screenshot_matches is None:
                raise RuntimeError("the X11 screenshot did not capture the rendered page")
        except BaseException:
            if process is not None:
                stop_photon(process)
            output = log_path.read_text(encoding="utf-8", errors="replace") if log_path.exists() else ""
            if output:
                print("Photon output (last 12000 characters):")
                print(output[-12_000:])
            raise
        finally:
            server.shutdown()
            server.server_close()
            server_thread.join(timeout=2)


if __name__ == "__main__":
    run_smoke()
