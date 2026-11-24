#!/usr/bin/env python3
"""Check relative Markdown links resolve inside the repository.

Only local paths are checked. External URLs, mail links, and pure anchors are
ignored. The exit status is non-zero when a target is missing.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
SKIP_PREFIXES = ("http://", "https://", "mailto:", "#")


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    checked = 0
    missing: list[tuple[Path, str]] = []

    for path in sorted(root.rglob("*.md")):
        if any(part in {"target", ".git", "node_modules"} for part in path.parts):
            continue
        text = path.read_text(encoding="utf-8")
        for target in LINK.findall(text):
            if target.startswith(SKIP_PREFIXES):
                continue
            clean = target.split("#", 1)[0]
            if not clean:
                continue
            checked += 1
            resolved = (path.parent / clean).resolve()
            if not resolved.exists():
                missing.append((path.relative_to(root), target))

    for path, target in missing:
        print(f"missing: {path} -> {target}")
    print(f"checked {checked} relative links in "
          f"{sum(1 for _ in root.rglob('*.md'))} Markdown files")
    if missing:
        print(f"FAILED: {len(missing)} missing target(s)")
        return 1
    print("OK: every relative link resolves")
    return 0


if __name__ == "__main__":
    sys.exit(main())
