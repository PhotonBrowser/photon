#!/usr/bin/env python3
"""Exercise Photon tab cleanup with isolated, memory-heavy local pages."""

from __future__ import annotations

import argparse
import http.server
import json
import os
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse


ROOT = Path(__file__).resolve().parents[1]
PROTOCOL_VERSION = 1
PRESSURE_RETRY_SECONDS = 5
MIB = 1024 * 1024
GPU_CACHE_PATTERN = re.compile(
    r"Photon memory-pressure GPU cache: (\d+) -> (\d+) bytes"
)


class StressFailure(RuntimeError):
    pass


class FixtureHandler(http.server.BaseHTTPRequestHandler):
    mib_per_tab = 16

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        query = parse_qs(urlparse(self.path).query)
        tab_number = query.get("tab", ["?"])[0]
        page = f"""<!doctype html>
<meta charset="utf-8">
<title>Photon memory fixture</title>
<style>
  html, body {{ margin: 0; width: 100%; height: 100%; background: #16181d; }}
  canvas {{ width: 100vw; height: 100vh; display: block; }}
</style>
<canvas width="1024" height="1024"></canvas>
<script>
(() => {{
  const transient = [];
  const bytes = {self.mib_per_tab} * 1024 * 1024;
  for (let chunk = 0; chunk < bytes; chunk += 2 * 1024 * 1024) {{
    const buffer = new Uint8Array(Math.min(2 * 1024 * 1024, bytes - chunk));
    for (let offset = 0; offset < buffer.length; offset += 4096)
      buffer[offset] = (offset / 4096 + chunk + {len(tab_number)}) & 255;
    transient.push(buffer);
  }}
  window.__photonMemoryFixture = transient;
  document.addEventListener('visibilitychange', () => {{
    if (document.hidden)
      window.__photonMemoryFixture = null;
  }});

  const canvas = document.querySelector('canvas');
  const context = canvas.getContext('2d');
  context.fillStyle = '#3975c6';
  context.fillRect(0, 0, canvas.width, canvas.height);
  for (let y = 0; y < canvas.height; y += 64) {{
    context.fillStyle = y % 128 ? '#e5a14c' : '#68bd92';
    context.fillRect(0, y, canvas.width, 32);
  }}
  document.title = 'PHOTON_MEMORY_READY_{tab_number}';
  document.body.dataset.ready = 'true';
}})();
</script>""".encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(page)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(page)

    def log_message(self, _format: str, *_args: object) -> None:
        pass


class AutomationClient:
    def __init__(self, socket_path: Path, token: str):
        self.socket_path = socket_path
        self.token = token
        self.request_id = 0

    def call(self, command: str, params: dict[str, Any] | None = None) -> Any:
        self.request_id += 1
        request: dict[str, Any] = {
            "protocol_version": PROTOCOL_VERSION,
            "request_id": str(self.request_id),
            "authentication": self.token,
            "command": command,
        }
        if params is not None:
            request["params"] = params

        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(15)
            connection.connect(str(self.socket_path))
            connection.sendall(json.dumps(request, separators=(",", ":")).encode() + b"\n")
            response_file = connection.makefile("rb")
            response_bytes = response_file.readline(256 * 1024 + 1)
        if len(response_bytes) > 256 * 1024:
            raise StressFailure(f"Photon automation response for {command} exceeded 256 KiB")
        try:
            response = json.loads(response_bytes)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise StressFailure(f"invalid Photon automation response for {command}: {error}") from error
        if not response.get("ok"):
            detail = response.get("error", {})
            raise StressFailure(
                f"Photon automation command {command} failed: "
                f"{detail.get('code', 'unknown_error')}: {detail.get('message', 'no message')}"
            )
        return response.get("result")


class MemoryPressureScenario:
    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.photon = args.photon.resolve()
        if not self.photon.is_file():
            raise StressFailure(f"Photon executable does not exist: {self.photon}")
        if sys.platform != "darwin":
            raise StressFailure("the memory pressure stress runner currently requires macOS")
        if args.tabs < 6:
            raise StressFailure("--tabs must be at least 6 to exercise a retried cleanup batch")
        if args.mib_per_tab < 1:
            raise StressFailure("--mib-per-tab must be at least 1")
        if args.minimum_webcontent_reclaimed_mib < 0:
            raise StressFailure("--minimum-webcontent-reclaimed-mib cannot be negative")

        self.token = secrets.token_hex(32)
        self.session_root = Path(
            tempfile.mkdtemp(prefix="photon-mem-", dir="/tmp")
        )
        self.home = self.session_root / "Home"
        self.profile = self.session_root / "Profile"
        self.automation_dir = self.session_root / "A"
        for directory in (self.home, self.profile, self.automation_dir):
            directory.mkdir(mode=0o700)
            directory.chmod(0o700)
        self.session_root.chmod(0o700)
        self.socket_path = self.automation_dir / "s"
        token_path = self.automation_dir / "token"
        token_path.write_text(self.token + "\n", encoding="ascii")
        token_path.chmod(0o600)
        (self.profile / "config.toml").write_text(
            "tab_sleep_mode = \"off\"\nrestore_session = false\n", encoding="utf-8"
        )
        (self.profile / "config.toml").chmod(0o600)

        self.client = AutomationClient(self.socket_path, self.token)
        self.log_path = self.session_root / "photon.log"
        self.process: subprocess.Popen[bytes] | None = None
        self.servers: list[http.server.ThreadingHTTPServer] = []
        self.server_threads: list[threading.Thread] = []
        self.fixture_tabs: list[int] = []
        self.active_tab: int | None = None

    def run(self) -> dict[str, Any]:
        try:
            self._start_fixture_server()
            self._start_photon()
            self._wait_for_automation()
            version = self.client.call("get_version")
            capabilities = version.get("capabilities", [])
            if "memory_pressure_test_v1" not in capabilities:
                raise StressFailure("this Photon build does not advertise memory_pressure_test_v1")

            window_id = self._wait_for_window()
            self._open_fixture_tabs(window_id)
            if not self.fixture_tabs:
                raise StressFailure("the fixture did not create any Photon tabs")
            self.active_tab = self.fixture_tabs[0]
            self.client.call("activate_tab", {"tab_id": self.active_tab})
            self._wait_for_tabs(lambda tabs: self._all_tabs_loaded(tabs), "all fixture tabs loaded")
            self._assert_no_fixture_tabs_sleeping("initial state")
            baseline = self._snapshot(window_id)
            self._report("baseline", baseline)

            self.client.call("set_memory_pressure", {"level": "warning"})
            warning_tabs = self._fixture_tab_map(window_id)
            if any(tab.get("sleeping") for tab in warning_tabs.values()):
                raise StressFailure("warning pressure discarded a fixture tab")
            self._wait(2.5, "warning memory sample")
            warning = self._snapshot(window_id)
            warning_reclamation = self._check_warning_reclamation(baseline, warning)
            self._report("warning", warning)

            self.client.call("set_memory_pressure", {"level": "normal"})
            self._wait(PRESSURE_RETRY_SECONDS + 1, "warning retry cancellation")
            self._assert_no_fixture_tabs_sleeping("after warning pressure returned to normal")
            cancelled_warning = self._snapshot(window_id)
            self._report("warning_cancelled", cancelled_warning)

            self.client.call("set_memory_pressure", {"level": "critical"})
            first_batch = self._wait_for_sleeping_count(
                min(4, len(self.fixture_tabs) - 1),
                self.args.cleanup_timeout,
                "first critical cleanup batch",
                window_id,
            )
            self._assert_active_loaded(first_batch, "first critical cleanup batch")
            first_count = self._sleeping_count(first_batch)
            if first_count != min(4, len(self.fixture_tabs) - 1):
                raise StressFailure(
                    f"critical pressure discarded {first_count} hidden tabs; "
                    f"expected {min(4, len(self.fixture_tabs) - 1)} in the first batch"
                )
            self._report("critical_first_batch", self._snapshot(window_id))

            self.client.call("set_memory_pressure", {"level": "normal"})
            self._wait(PRESSURE_RETRY_SECONDS + 1, "critical retry cancellation")
            after_cancelled_critical = self._fixture_tab_map(window_id)
            self._assert_active_loaded(after_cancelled_critical, "after critical retry cancellation")
            if self._sleeping_count(after_cancelled_critical) != first_count:
                raise StressFailure("returning to normal did not stop the pending critical retry")
            self._report("critical_retry_cancelled", self._snapshot(window_id))

            self.client.call("set_memory_pressure", {"level": "critical"})
            expected_sleeping = len(self.fixture_tabs) - 1
            final_tabs = self._wait_for_sleeping_count(
                expected_sleeping,
                self.args.cleanup_timeout,
                "critical pressure retries",
                window_id,
            )
            self._assert_active_loaded(final_tabs, "completed critical cleanup")
            if self._sleeping_count(final_tabs) != expected_sleeping:
                raise StressFailure("critical pressure did not discard every eligible hidden fixture tab")

            self._wait(2, "memory metrics settling")
            final = self._snapshot(window_id)
            self._report("critical_complete", final)
            pressure = self._check_memory_reclamation(baseline, final)
            gpu = self._check_gpu_reclamation()

            self.client.call("set_memory_pressure", {"level": "system"})
            return {
                "photon": str(self.photon),
                "fixture_tabs": len(self.fixture_tabs),
                "active_tab": self.active_tab,
                "warning_kept_tabs_loaded": True,
                "critical_discarded_only_hidden_tabs": True,
                "normal_stopped_pending_retries": True,
                "warning_memory": warning_reclamation,
                "memory": pressure,
                "gpu_cache": gpu,
            }
        finally:
            if sys.exc_info()[0] is not None:
                self._print_log_tail()
            self._stop_photon()
            self._stop_fixture_server()
            # Keep the isolated log available only until reporting is complete.
            if self.session_root.exists():
                shutil.rmtree(self.session_root, ignore_errors=True)

    def _start_fixture_server(self) -> None:
        handler = type("ConfiguredFixtureHandler", (FixtureHandler,), {})
        handler.mib_per_tab = self.args.mib_per_tab
        for _ in range(self.args.tabs):
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
            server.daemon_threads = True
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            self.servers.append(server)
            self.server_threads.append(thread)

    def _start_photon(self) -> None:
        build_profile = self.photon.parent.name
        if build_profile not in {"debug", "release"}:
            build_profile = "debug"
        helper_directory = ROOT / "build" / f"app-{build_profile}"
        engine_directory = ROOT / "build" / f"engine-{build_profile}"
        required = (
            helper_directory / "WebContent",
            engine_directory / "lib" / "liblagom-photonembedder.dylib",
        )
        missing = [str(path) for path in required if not path.is_file()]
        if missing:
            raise StressFailure(
                "Photon's Engine helpers are missing; run ./photon build first: "
                + ", ".join(missing)
            )

        service = _presentation_service_name(ROOT)
        target = f"gui/{os.getuid()}/{service}"
        registered = subprocess.run(
            ["launchctl", "print", target],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if registered.returncode != 0:
            raise StressFailure(
                "Photon's presentation service is not registered; run './photon run' "
                "once to initialize this development build"
            )

        environment = os.environ.copy()
        environment.update(
            {
                "HOME": str(self.home),
                "PHOTON_PROFILE_DIR": str(self.profile),
                "PHOTON_HELPER_DIRECTORY": str(helper_directory),
                "PHOTON_PRESENTATION_XPC_SERVICE": service,
                "PHOTON_PRESENTATION_CHANNEL_ID": f"memory-pressure-{self.token[:16]}",
                "DYLD_LIBRARY_PATH": ":".join(
                    str(path)
                    for path in (engine_directory / "lib64", engine_directory / "lib")
                    if path.is_dir()
                ),
            }
        )
        with self.log_path.open("wb") as log_file:
            self.process = subprocess.Popen(
                [str(self.photon), "--automation-socket", str(self.socket_path)],
                cwd=ROOT,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=log_file,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )

    def _wait_for_automation(self) -> None:
        deadline = time.monotonic() + self.args.startup_timeout
        while time.monotonic() < deadline:
            if self.process is None or self.process.poll() is not None:
                raise StressFailure("Photon exited before automation became available")
            try:
                self.client.call("ping")
                return
            except (OSError, StressFailure):
                time.sleep(0.2)
        raise StressFailure("timed out waiting for the Photon automation socket")

    def _wait_for_window(self) -> int:
        deadline = time.monotonic() + self.args.startup_timeout
        while time.monotonic() < deadline:
            result = self.client.call("list_windows")
            windows = result.get("windows", [])
            if windows:
                return int(windows[0]["window_id"])
            time.sleep(0.2)
        raise StressFailure("Photon opened no browser window before the startup timeout")

    def _open_fixture_tabs(self, window_id: int) -> None:
        for index in range(self.args.tabs):
            # A unique port gives each local page a distinct origin on macOS loopback.
            port = self.servers[index].server_address[1]
            host = "127.0.0.1"
            url = f"http://{host}:{port}/?tab={index}"
            created = self.client.call("new_tab", {"window_id": window_id, "url": url})
            tab_id = int(created["tab_id"])
            self.fixture_tabs.append(tab_id)
            self._wait_for_tabs(
                lambda tabs, expected=tab_id, number=index: (
                    expected in tabs
                    and tabs[expected].get("engine_load_finished") is True
                    and tabs[expected].get("title") == f"PHOTON_MEMORY_READY_{number}"
                ),
                f"fixture page {index + 1}/{self.args.tabs} loaded",
            )
            print(f"Loaded fixture {index + 1}/{self.args.tabs} ({self.args.mib_per_tab} MiB)")

    def _all_tabs_loaded(self, tabs: dict[int, dict[str, Any]]) -> bool:
        return all(
            tab_id in tabs
            and tabs[tab_id].get("engine_load_finished") is True
            and tabs[tab_id].get("sleeping") is False
            for tab_id in self.fixture_tabs
        )

    def _wait_for_tabs(self, condition: Any, label: str) -> dict[int, dict[str, Any]]:
        deadline = time.monotonic() + self.args.page_timeout
        while time.monotonic() < deadline:
            tabs = self._fixture_tab_map()
            if condition(tabs):
                return tabs
            if self.process is None or self.process.poll() is not None:
                raise StressFailure(f"Photon exited while waiting for {label}")
            time.sleep(0.25)
        raise StressFailure(f"timed out waiting for {label}")

    def _fixture_tab_map(self, window_id: int | None = None) -> dict[int, dict[str, Any]]:
        result = self.client.call("list_tabs", {"window_id": window_id})
        all_tabs = {int(tab["tab_id"]): tab for tab in result.get("tabs", [])}
        return {tab_id: all_tabs[tab_id] for tab_id in self.fixture_tabs if tab_id in all_tabs}

    def _wait_for_sleeping_count(
        self,
        expected: int,
        timeout: float,
        label: str,
        window_id: int,
    ) -> dict[int, dict[str, Any]]:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            tabs = self._fixture_tab_map(window_id)
            if self._sleeping_count(tabs) >= expected:
                return tabs
            if self.process is None or self.process.poll() is not None:
                raise StressFailure(f"Photon exited during {label}")
            time.sleep(0.2)
        raise StressFailure(
            f"timed out during {label}: {self._sleeping_count(self._fixture_tab_map(window_id))} "
            f"of {expected} expected tabs were sleeping"
        )

    def _assert_no_fixture_tabs_sleeping(self, label: str) -> None:
        tabs = self._fixture_tab_map()
        sleeping = [tab_id for tab_id, tab in tabs.items() if tab.get("sleeping")]
        if sleeping:
            raise StressFailure(f"{label} unexpectedly slept fixture tabs: {sleeping}")

    def _assert_active_loaded(self, tabs: dict[int, dict[str, Any]], label: str) -> None:
        if self.active_tab is None or self.active_tab not in tabs:
            raise StressFailure(f"{label}: the active fixture tab disappeared")
        if tabs[self.active_tab].get("sleeping"):
            raise StressFailure(f"{label}: critical pressure discarded the active tab")

    @staticmethod
    def _sleeping_count(tabs: dict[int, dict[str, Any]]) -> int:
        return sum(bool(tab.get("sleeping")) for tab in tabs.values())

    def _snapshot(self, window_id: int) -> dict[str, Any]:
        tabs = self._fixture_tab_map(window_id)
        process_result = self.client.call("list_processes")
        processes = process_result.get("processes", [])
        pids = [int(process["pid"]) for process in processes]
        rss_by_pid = _sample_rss(pids)
        summarized = []
        for process in processes:
            pid = int(process["pid"])
            summarized.append(
                {
                    "pid": pid,
                    "role": process.get("role", "unknown"),
                    "rss_bytes": rss_by_pid.get(pid),
                }
            )
        web_content = sum(
            int(process["rss_bytes"])
            for process in summarized
            if process["role"] == "web_content" and process["rss_bytes"] is not None
        )
        compositor = sum(
            int(process["rss_bytes"])
            for process in summarized
            if process["role"] == "compositor" and process["rss_bytes"] is not None
        )
        return {
            "sleeping_tabs": self._sleeping_count(tabs),
            "loaded_tabs": sum(not bool(tab.get("sleeping")) for tab in tabs.values()),
            "web_content_processes": sum(
                process["role"] == "web_content" for process in summarized
            ),
            "web_content_rss_bytes": web_content,
            "compositor_rss_bytes": compositor,
            "tab_memory_bytes": sum(
                int(tab["memory_bytes"])
                for tab in tabs.values()
                if isinstance(tab.get("memory_bytes"), int)
            ),
            "managed_heap_bytes": sum(
                int(tab["managed_heap_bytes"])
                for tab in tabs.values()
                if isinstance(tab.get("managed_heap_bytes"), int)
            ),
            "tabs": [
                {
                    "tab_id": tab_id,
                    "active": bool(tab.get("active")),
                    "sleeping": bool(tab.get("sleeping")),
                    "memory_bytes": tab.get("memory_bytes"),
                    "managed_heap_bytes": tab.get("managed_heap_bytes"),
                }
                for tab_id, tab in tabs.items()
            ],
            "processes": summarized,
        }

    @staticmethod
    def _report(label: str, snapshot: dict[str, Any]) -> None:
        print(
            f"{label}: loaded={snapshot['loaded_tabs']} sleeping={snapshot['sleeping_tabs']} "
            f"WebContent={snapshot['web_content_processes']} "
            f"WebContent RSS={_format_bytes(snapshot['web_content_rss_bytes'])} "
            f"Compositor RSS={_format_bytes(snapshot['compositor_rss_bytes'])} "
            f"tab estimate={_format_optional_bytes(snapshot['tab_memory_bytes'])}"
        )

    def _check_memory_reclamation(
        self, baseline: dict[str, Any], final: dict[str, Any]
    ) -> dict[str, Any]:
        reclaimed = baseline["web_content_rss_bytes"] - final["web_content_rss_bytes"]
        minimum = self.args.minimum_webcontent_reclaimed_mib * MIB
        if reclaimed < minimum:
            raise StressFailure(
                "WebContent RSS did not fall by the configured minimum: "
                f"baseline={_format_bytes(baseline['web_content_rss_bytes'])}, "
                f"final={_format_bytes(final['web_content_rss_bytes'])}, "
                f"reclaimed={_format_bytes(max(0, reclaimed))}, "
                f"required={_format_bytes(minimum)}"
            )
        return {
            "web_content_rss_before_bytes": baseline["web_content_rss_bytes"],
            "web_content_rss_after_bytes": final["web_content_rss_bytes"],
            "web_content_rss_reclaimed_bytes": reclaimed,
            "web_content_processes_before": baseline["web_content_processes"],
            "web_content_processes_after": final["web_content_processes"],
            "tab_memory_estimate_before_bytes": baseline["tab_memory_bytes"],
            "tab_memory_estimate_after_bytes": final["tab_memory_bytes"],
        }

    def _check_warning_reclamation(
        self, baseline: dict[str, Any], warning: dict[str, Any]
    ) -> dict[str, Any]:
        managed_heap_reclaimed = (
            baseline["managed_heap_bytes"] - warning["managed_heap_bytes"]
        )
        tab_memory_reclaimed = baseline["tab_memory_bytes"] - warning["tab_memory_bytes"]
        web_content_reclaimed = (
            baseline["web_content_rss_bytes"] - warning["web_content_rss_bytes"]
        )
        gpu_entries = self._gpu_cache_entries()
        gpu_reclaimed = sum(
            max(0, entry["before_bytes"] - entry["after_bytes"])
            for entry in gpu_entries
        )
        if managed_heap_reclaimed > 0:
            metric = "managed_heap_bytes"
            reclaimed = managed_heap_reclaimed
        elif tab_memory_reclaimed > 0:
            metric = "tab_memory_bytes"
            reclaimed = tab_memory_reclaimed
        elif gpu_reclaimed > 0:
            metric = "gpu_cache_bytes"
            reclaimed = gpu_reclaimed
        elif web_content_reclaimed > 0:
            metric = "web_content_rss_bytes"
            reclaimed = web_content_reclaimed
        else:
            if self.args.allow_unobserved_gpu_reclaim and not gpu_entries:
                print("Warning memory: not measurable (GPU metrics explicitly waived)")
                return {"verified": False, "reason": "no memory counters were observable"}
            elif gpu_entries and all(entry["before_bytes"] == 0 for entry in gpu_entries):
                print("Warning memory: no hidden heap or unlocked GPU scratch bytes to reclaim")
                return {
                    "verified": False,
                    "reason": "all reported reclaimable counters were already zero",
                }
            else:
                raise StressFailure(
                    "warning pressure kept every page loaded but no process or GPU memory "
                    "counter decreased"
                )
        gpu_before = max((entry["before_bytes"] for entry in gpu_entries), default=0)
        gpu_after = max((entry["after_bytes"] for entry in gpu_entries), default=0)
        if metric in baseline:
            before_bytes = baseline[metric]
            after_bytes = warning[metric]
        else:
            before_bytes = gpu_before
            after_bytes = gpu_after
        return {
            "verified": True,
            "metric": metric,
            "before_bytes": before_bytes,
            "after_bytes": after_bytes,
            "reclaimed_bytes": reclaimed,
            "loaded_tabs_before": baseline["loaded_tabs"],
            "loaded_tabs_after": warning["loaded_tabs"],
        }

    def _gpu_cache_entries(self) -> list[dict[str, int]]:
        log = self.log_path.read_text(errors="replace") if self.log_path.exists() else ""
        return [
            {"before_bytes": int(before), "after_bytes": int(after)}
            for before, after in GPU_CACHE_PATTERN.findall(log)
        ]

    def _check_gpu_reclamation(self) -> dict[str, Any]:
        entries = self._gpu_cache_entries()
        reclaimed = sum(
            max(0, entry["before_bytes"] - entry["after_bytes"]) for entry in entries
        )
        if not entries:
            if not self.args.allow_unobserved_gpu_reclaim:
                raise StressFailure(
                    "no GPU cache purge metrics appeared in Photon logs; use a debug build "
                    "or pass --allow-unobserved-gpu-reclaim to record this as unverified"
                )
            print("GPU cache: unobserved (allowed by --allow-unobserved-gpu-reclaim)")
        elif reclaimed == 0 and any(entry["before_bytes"] > 0 for entry in entries):
            raise StressFailure(
                "the GPU cache had resources before pressure, but no purge reduced its size"
            )
        elif reclaimed == 0:
            print("GPU cache: no unlocked scratch resources were present to reclaim")
        else:
            print(f"GPU cache: released {_format_bytes(reclaimed)} across {len(entries)} purge(s)")
        return {"purges": entries, "reclaimed_bytes": reclaimed, "observed": bool(entries)}

    @staticmethod
    def _wait(seconds: float, label: str) -> None:
        print(f"Waiting {seconds:.1f}s for {label}...")
        time.sleep(seconds)

    def _stop_photon(self) -> None:
        if self.process is None:
            return
        if self.process.poll() is None:
            try:
                self.client.call("set_memory_pressure", {"level": "normal"})
            except (OSError, StressFailure):
                pass
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.process.wait(timeout=5)

    def _stop_fixture_server(self) -> None:
        for server in self.servers:
            server.shutdown()
            server.server_close()
        for thread in self.server_threads:
            thread.join(timeout=2)

    def _print_log_tail(self) -> None:
        if not self.log_path.exists():
            return
        lines = self.log_path.read_text(errors="replace").splitlines()
        if lines:
            print("Photon log tail:", file=sys.stderr)
            print("\n".join(lines[-60:]), file=sys.stderr)


def _presentation_service_name(root: Path) -> str:
    value = 0xCBF29CE484222325
    for byte in os.fsencode(root.resolve()):
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"org.photonbrowser.photon.presentation.u{os.getuid()}.h{value:016x}.gpui-ce"


def _sample_rss(pids: list[int]) -> dict[int, int]:
    if not pids:
        return {}
    result = subprocess.run(
        ["ps", "-p", ",".join(map(str, pids)), "-o", "pid=,rss="],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return {}
    rss: dict[int, int] = {}
    for line in result.stdout.splitlines():
        fields = line.split()
        if len(fields) == 2:
            rss[int(fields[0])] = int(fields[1]) * 1024
    return rss


def _format_bytes(value: int) -> str:
    return f"{value / MIB:.1f} MiB"


def _format_optional_bytes(value: int) -> str:
    return _format_bytes(value) if value > 0 else "unavailable"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run isolated Photon memory-pressure cleanup and reclamation checks."
    )
    parser.add_argument("--photon", type=Path, required=True, help="built Photon executable")
    parser.add_argument(
        "--tabs", type=int, default=6, help="number of heavy fixture tabs (default: 6)"
    )
    parser.add_argument(
        "--mib-per-tab", type=int, default=8, help="typed-array MiB per page (default: 8)"
    )
    parser.add_argument("--startup-timeout", type=float, default=60)
    parser.add_argument("--page-timeout", type=float, default=90)
    parser.add_argument("--cleanup-timeout", type=float, default=30)
    parser.add_argument(
        "--minimum-webcontent-reclaimed-mib",
        type=int,
        default=16,
        help="minimum WebContent RSS drop required for success (default: 16 MiB)",
    )
    parser.add_argument(
        "--allow-unobserved-gpu-reclaim",
        action="store_true",
        help="report missing GPU cache metrics without failing the run",
    )
    return parser.parse_args()


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(line_buffering=True)
    args = parse_args()
    try:
        result = MemoryPressureScenario(args).run()
    except (OSError, StressFailure, subprocess.SubprocessError) as error:
        print(f"Memory pressure stress failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    print("Memory pressure stress passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
