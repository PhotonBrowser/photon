#!/usr/bin/env python3
"""Render a packaged Photon archive in a runtime-only container without a checkout."""

from __future__ import annotations

import argparse
import subprocess
import tarfile
import tempfile
from pathlib import Path


def run_archive(archive: Path, image: str, evidence: Path, desktop: bool = False) -> None:
    archive = archive.resolve(strict=True)
    evidence = evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    smoke = Path(__file__).resolve().with_name("linux_render_smoke.py")
    with tempfile.TemporaryDirectory(prefix="photon package smoke ") as temporary:
        extracted = Path(temporary)
        with tarfile.open(archive) as package:
            package.extractall(extracted, filter="data")
        executables = list(extracted.glob("*/bin/Photon"))
        if len(executables) != 1 or not executables[0].is_file():
            raise ValueError("archive must contain exactly one package with bin/Photon")
        executable = Path("/package with spaces") / executables[0].relative_to(extracted)
        subprocess.run(
            [
                "docker", "run", "--rm", "--init", "--network", "none", "--shm-size", "1g",
                "--mount", f"type=bind,src={extracted},dst=/package with spaces,readonly",
                "--mount", f"type=bind,src={smoke},dst=/smoke/linux_render_smoke.py,readonly",
                "--mount", f"type=bind,src={evidence},dst=/evidence",
                "--env", "PHOTON_RENDER_SCREENSHOT=/evidence/linux-package-render.png",
                "--env", "PHOTON_RENDER_LOG=/evidence/photon.log",
                image,
                "xvfb-run", "-a", "-s", "-screen 0 1280x800x24 +extension GLX +render -noreset",
                "python3", "/smoke/linux_render_smoke.py", "--executable", str(executable),
                *(["--desktop"] if desktop else []),
            ],
            check=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--image", default="photon-linux220-runtime")
    parser.add_argument("--evidence", type=Path, default=Path("build/package-smoke"))
    parser.add_argument("--desktop", action="store_true", help="exercise the installed default URL handler")
    arguments = parser.parse_args()
    run_archive(arguments.archive, arguments.image, arguments.evidence, arguments.desktop)
