#!/usr/bin/env python3
"""Fail if GitHub Actions workflow YAML has duplicate top-level keys.

Also fail if include_str! files would be missing from the Docker context.
GitHub rejects bad workflow YAML as 0-job failures named after the path
(e.g. `.github/workflows/auto-research.yml`) on every push.
Docker publish compiles include_str! from static/; .dockerignore *.md
dropped grok-bot.md and turned main red while ci.yml stayed green.
"""

from __future__ import annotations

from collections import Counter
from pathlib import Path
import fnmatch
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"
INCLUDE_STR = re.compile(r'include_str!\("([^"]+)"\)')


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


def dockerignore_rules() -> list[tuple[bool, str]]:
    path = ROOT / ".dockerignore"
    if not path.is_file():
        return []
    rules: list[tuple[bool, str]] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        negate = line.startswith("!")
        pat = line[1:] if negate else line
        rules.append((negate, pat))
    return rules


def docker_match(pat: str, rel: str) -> bool:
    rel = rel.lstrip("./")
    name = Path(rel).name
    return bool(
        fnmatch.fnmatch(rel, pat)
        or fnmatch.fnmatch(name, pat)
        or fnmatch.fnmatch(rel, f"**/{pat}")
    )


def is_dockerignored(rel: str, rules: list[tuple[bool, str]]) -> bool:
    ignored = False
    for negate, pat in rules:
        if docker_match(pat, rel):
            ignored = not negate
    return ignored


def lint_include_str() -> list[str]:
    errors: list[str] = []
    rules = dockerignore_rules()
    seen: set[Path] = set()
    for source in (ROOT / "src").rglob("*.rs"):
        text = source.read_text(encoding="utf-8")
        for match in INCLUDE_STR.finditer(text):
            target = (source.parent / match.group(1)).resolve()
            if target in seen:
                continue
            seen.add(target)
            try:
                rel = target.relative_to(ROOT).as_posix()
            except ValueError:
                errors.append(f"{source}: include_str! leaves the repo: {target}")
                continue
            if not target.is_file():
                errors.append(f"{source}: include_str! missing {rel}")
                continue
            if is_dockerignored(rel, rules):
                errors.append(
                    f"{source}: include_str! {rel} is excluded by .dockerignore "
                    "(Docker publish will fail while cargo test still passes)"
                )
    return errors


def main() -> int:
    files = sorted(WORKFLOWS.glob("*.yml")) + sorted(WORKFLOWS.glob("*.yaml"))
    if not files:
        print(f"no workflow files under {WORKFLOWS}", file=sys.stderr)
        return 1
    errors: list[str] = []
    for path in files:
        errors.extend(lint(path))
    errors.extend(lint_include_str())
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"ok: {len(files)} workflow files, include_str! files in Docker context")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
