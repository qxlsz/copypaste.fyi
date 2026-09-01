#!/usr/bin/env python3
"""Fail if GitHub Actions workflow YAML has duplicate top-level keys.

GitHub rejects those files as 0-job failures named after the path
(e.g. `.github/workflows/auto-research.yml`) on every push.
"""

from __future__ import annotations

from collections import Counter
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"


def top_level_keys(text: str) -> list[str]:
    keys: list[str] = []
    for line in text.splitlines():
        if not line or line[0] in " \t#-":
            continue
        if ":" not in line:
            continue
        keys.append(line.split(":", 1)[0].strip())
    return keys


def lint(path: Path) -> list[str]:
    errors: list[str] = []
    text = path.read_text(encoding="utf-8")
    if not text.strip():
        return [f"{path}: empty workflow file"]
    keys = top_level_keys(text)
    dupes = [key for key, count in Counter(keys).items() if count > 1]
    if dupes:
        errors.append(f"{path}: duplicate top-level keys: {', '.join(dupes)}")
    for required in ("name", "on", "jobs"):
        if required not in keys:
            errors.append(f"{path}: missing top-level `{required}`")
    return errors


def main() -> int:
    files = sorted(WORKFLOWS.glob("*.yml")) + sorted(WORKFLOWS.glob("*.yaml"))
    if not files:
        print(f"no workflow files under {WORKFLOWS}", file=sys.stderr)
        return 1
    errors: list[str] = []
    for path in files:
        errors.extend(lint(path))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"ok: {len(files)} workflow files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
