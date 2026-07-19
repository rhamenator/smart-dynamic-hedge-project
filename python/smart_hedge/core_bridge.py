from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

from .config import project_root, resolve_project_path


class CoreError(RuntimeError):
    pass


def default_binary_path() -> Path:
    name = "smart_dynamic_hedge.exe" if os.name == "nt" else "smart_dynamic_hedge"
    return project_root() / "build" / name


def resolve_binary(config: dict[str, Any]) -> Path:
    raw = str(config.get("core", {}).get("binary", "")).strip()
    return resolve_project_path(config, raw) if raw else default_binary_path()


def build_core(config: dict[str, Any]) -> Path:
    root = project_root()
    build_dir = root / "build"
    build_dir.mkdir(parents=True, exist_ok=True)
    cmake = shutil.which("cmake")
    if cmake:
        subprocess.run(
            [cmake, "-S", str(root), "-B", str(build_dir), "-DCMAKE_BUILD_TYPE=Release"],
            check=True,
            timeout=120,
        )
        subprocess.run(
            [cmake, "--build", str(build_dir), "--config", "Release", "-j"],
            check=True,
            timeout=180,
        )
    else:
        compiler = shutil.which("g++") or shutil.which("clang++")
        if not compiler:
            raise CoreError("cmake, g++, or clang++ is required to build the C++ core")
        output = default_binary_path()
        subprocess.run(
            [
                compiler,
                "-std=c++17",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Wpedantic",
                str(root / "cpp" / "smart_dynamic_hedge.cpp"),
                "-o",
                str(output),
            ],
            check=True,
            timeout=180,
        )
    binary = resolve_binary(config)
    # Multi-config Windows generators place Release binaries in a subdirectory.
    if not binary.exists() and os.name == "nt":
        candidate = project_root() / "build" / "Release" / "smart_dynamic_hedge.exe"
        if candidate.exists():
            binary = candidate
    if not binary.exists():
        raise CoreError(f"build completed but core binary was not found at {binary}")
    return binary


def ensure_core(config: dict[str, Any]) -> Path:
    binary = resolve_binary(config)
    if binary.exists():
        return binary
    if bool(config.get("core", {}).get("auto_build", True)):
        return build_core(config)
    raise CoreError(f"core binary not found: {binary}; run `smart-hedge build-core`")


def run_core(config: dict[str, Any], contract: dict[str, Any], spot: float) -> dict[str, Any]:
    binary = ensure_core(config)
    command = [
        str(binary),
        "--spot",
        str(float(spot)),
        "--strike",
        str(float(contract["strike"])),
        "--rate",
        str(float(contract.get("rate", 0.0))),
        "--dividend-yield",
        str(float(contract.get("dividend_yield", 0.0))),
        "--vol",
        str(float(contract["implied_volatility"])),
        "--days",
        str(float(contract["days_to_expiry"])),
        "--type",
        str(contract.get("option_type", "call")),
        "--style",
        str(contract.get("exercise_style", "american")),
        "--contracts",
        str(int(contract.get("contracts", 0))),
        "--multiplier",
        str(float(contract.get("multiplier", 100.0))),
        "--current-shares",
        str(float(contract.get("current_shares", 0.0))),
        "--tree-steps",
        str(int(config.get("core", {}).get("tree_steps", 600))),
        "--no-trade-band",
        str(float(contract.get("base_no_trade_band_shares", 0.0))),
        "--json",
    ]
    timeout = float(config.get("core", {}).get("timeout_seconds", 12.0))
    try:
        completed = subprocess.run(
            command,
            check=False,
            text=True,
            capture_output=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        raise CoreError(f"C++ core timed out after {timeout}s") from exc
    if completed.returncode != 0:
        raise CoreError(completed.stderr.strip() or f"C++ core exited {completed.returncode}")
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise CoreError("C++ core returned invalid JSON") from exc
    if not isinstance(payload, dict) or "hedge" not in payload or "greeks" not in payload:
        raise CoreError("C++ core response is missing required fields")
    return payload
