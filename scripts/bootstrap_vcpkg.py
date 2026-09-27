#!/usr/bin/env python3
"""Bootstrap the vcpkg revision pinned by Photon Engine into an external build tree."""

import sys
from pathlib import Path

engine_source = Path(sys.argv[1]).resolve()
vcpkg_root = Path(sys.argv[2]).resolve()
sys.path.insert(0, str(engine_source / "Meta"))

from Utils.build_vcpkg import build_vcpkg  # noqa: E402

build_vcpkg(vcpkg_root)
