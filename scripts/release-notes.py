#!/usr/bin/env python3
"""Pull a single version's notes out of a Keep-a-Changelog CHANGELOG.md.

Usage:
    python3 scripts/release-notes.py CHANGELOG.md 0.0.1

Prints the section body (without its heading) to stdout and exits 0, or exits
1 when the changelog has no section for that version. Ported from the reference
Waku app's `scripts/changelog.ts`.
"""

import re
import sys


def heading_version(line: str):
    """The version token from a level-2 heading, or None.

    Handles `## [0.0.1] - 2026-09-14`, `## 0.0.1`, `## v0.0.1`, etc.
    """
    match = re.match(r"^##\s+(.+)$", line)  # level 2 only
    if not match:
        return None
    token = match.group(1).strip().split()[0]  # before any " - date"
    token = token.strip("[]")  # strip [ ]
    return token[1:] if token[:1] in ("v", "V") else token  # strip leading v


def extract_release_notes(changelog: str, version: str):
    lines = changelog.split("\n")

    start = None
    for i, line in enumerate(lines):
        if heading_version(line) == version:
            start = i + 1
            break
    if start is None:
        return None

    end = len(lines)
    for i in range(start, len(lines)):
        if re.match(r"^##\s+", lines[i]):
            end = i
            break

    body = "\n".join(lines[start:end]).strip()
    return body or None


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: release-notes.py <changelog> <version>", file=sys.stderr)
        return 2
    try:
        with open(sys.argv[1], encoding="utf-8") as fh:
            changelog = fh.read()
    except FileNotFoundError:
        return 1

    notes = extract_release_notes(changelog, sys.argv[2])
    if not notes:
        return 1
    print(notes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
