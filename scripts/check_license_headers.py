#!/usr/bin/env python3

# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# SPDX-License-Identifier: MPL-2.0

"""Verify that every source file carries the licence notice for its path.

MPL-2.0 is file-level copyleft, so the root LICENSE does not stand in for a
per-file notice. The sync server is AGPL-3.0 and must carry that notice
instead; mixing the two would misstate the licence of the code. Two kinds of
file are exempt: generated project scaffolding, and SQL migrations, which are
frozen once merged and are compiled into an already covered Rust source file.
"""

import subprocess
import sys
from pathlib import Path

EXTENSIONS = {
    "rs", "ts", "tsx", "js", "mjs", "go", "kt", "kts", "swift", "h", "mm",
    "css", "sh",
}
EXEMPT_DIRS = ("/src-tauri/gen/",)
AGPL_PREFIX = "apps/sync-server/"

MPL_ID = "SPDX-License-Identifier: MPL-2.0"
AGPL_ID = "SPDX-License-Identifier: AGPL-3.0-or-later"
NOTICE = "This Source Code Form is subject to the terms of the Mozilla Public"
# The notice must sit in the file's opening comment, where a reader and a
# scanner both look for it — not buried further down.
SCAN_LINES = 22


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    listing = subprocess.run(
        ["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True
    ).stdout.split()

    problems = []
    checked = 0
    # Files the index still lists but the working tree no longer has: a
    # deletion that has not been staged yet. Skipping them is right, but
    # skipping them quietly is not — a guard that reads "the file is gone" as
    # "the check passed" is a guard that stops guarding.
    absent = []
    for rel in listing:
        extension = rel.rsplit(".", 1)[-1] if "." in rel else ""
        if extension not in EXTENSIONS:
            continue
        if any(part in f"/{rel}" for part in EXEMPT_DIRS):
            continue
        if not (root / rel).is_file():
            absent.append(rel)
            continue

        checked += 1
        head = "\n".join(
            (root / rel).read_text(encoding="utf-8").split("\n")[:SCAN_LINES]
        )
        expected, other = (
            (AGPL_ID, MPL_ID) if rel.startswith(AGPL_PREFIX) else (MPL_ID, AGPL_ID)
        )
        if expected not in head:
            problems.append(f"{rel}: missing '{expected}' in the first {SCAN_LINES} lines")
        elif other in head:
            problems.append(f"{rel}: carries both licence notices")
        elif expected is MPL_ID and NOTICE not in head:
            problems.append(f"{rel}: SPDX identifier without the MPL notice text")

    if absent:
        print(
            f"note: {len(absent)} tracked path(s) are missing from the working tree "
            "and were not checked — stage the deletions so the index matches."
        )
        for rel in absent[:5]:
            print(f"  absent: {rel}")
        if len(absent) > 5:
            print(f"  ... and {len(absent) - 5} more")

    if problems:
        print(f"Licence header check failed for {len(problems)} of {checked} files:\n")
        for problem in problems:
            print(f"  {problem}")
        print("\nSee 'Every new source file needs a licence header' in CONTRIBUTING.md.")
        return 1

    print(f"Licence headers OK ({checked} files).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
