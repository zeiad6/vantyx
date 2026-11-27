#!/usr/bin/env python3
"""Validate OpenCode SKILL.md structure and duplicate IDs.

Usage:
    python3 scripts/validate_skills.py
    python3 scripts/validate_skills.py --root ~/.config/opencode/skills
"""

from __future__ import annotations

import argparse
import os
import re
import sys

REQUIRED_SECTIONS = [
    "When to use",
    "Required tools",
    "Safety constraints",
    "Workflow",
    "Validation and acceptance criteria",
    "Expected output",
    "Troubleshooting",
    "Examples",
]

FRONTMATTER = re.compile(r"^---\r?\n(.*?)\r?\n---\r?\n", re.DOTALL)


def iter_skills(root: str):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [
            name
            for name in dirnames
            if name not in {".git", "node_modules", "target", ".venv", "__pycache__"}
        ]
        if "SKILL.md" in filenames:
            yield os.path.basename(dirpath), os.path.join(dirpath, "SKILL.md")


def validate(root: str) -> int:
    errors: list[str] = []
    ids: dict[str, list[str]] = {}

    for skill_id, path in sorted(iter_skills(root)):
        ids.setdefault(skill_id, []).append(path)
        try:
            text = open(path, encoding="utf-8").read()
        except OSError as exc:
            errors.append(f"{skill_id}: cannot read {path}: {exc}")
            continue

        match = FRONTMATTER.match(text)
        if not match:
            errors.append(f"{skill_id}: missing YAML frontmatter")
            continue

        frontmatter = match.group(1)
        if not re.search(r"^name:\s*\S", frontmatter, re.MULTILINE):
            errors.append(f"{skill_id}: missing frontmatter name")
        if not re.search(r"^description:\s*\S", frontmatter, re.MULTILINE):
            errors.append(f"{skill_id}: missing frontmatter description")

        missing = [section for section in REQUIRED_SECTIONS if f"## {section}" not in text]
        if missing:
            errors.append(f"{skill_id}: missing sections: {', '.join(missing)}")

        if len(text.encode("utf-8")) < 2500:
            errors.append(f"{skill_id}: suspiciously short ({len(text.encode('utf-8'))} bytes)")

    for skill_id, paths in sorted(ids.items()):
        if len(paths) > 1:
            errors.append(f"duplicate ID {skill_id}: " + ", ".join(paths))

    print(f"Validated {len(ids)} skill(s) under {root}")
    if errors:
        print(f"FAILED with {len(errors)} error(s):")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("OK: frontmatter, required sections, size, and unique IDs are valid.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        default=os.path.expanduser("~/.config/opencode/skills"),
        help="skills root directory (default: ~/.config/opencode/skills)",
    )
    args = parser.parse_args()
    return validate(os.path.abspath(os.path.expanduser(args.root)))


if __name__ == "__main__":
    sys.exit(main())
