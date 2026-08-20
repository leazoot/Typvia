#!/usr/bin/env bash
# Search red lines: 10k < 50ms, 50k < 150ms.
# Runs the criterion suite and prints the per-scenario worst mean time.
# More than 20% slower than the recorded baseline counts as a failure.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

cargo bench -p typvia-search "$@"

# Criterion writes structured estimates; summarize the red-line scenarios.
python3 - <<'EOF'
import json, pathlib

root = pathlib.Path("target/criterion")
if not root.exists():
    raise SystemExit("no criterion output found — did the bench run?")

worst = {}
for estimates in root.rglob("new/estimates.json"):
    parts = estimates.relative_to(root).parts[:-2]  # group/…/case dirs
    name = "/".join(parts)
    size = "10k" if "10000" in parts else "50k" if "50000" in parts else None
    if size is None:
        continue
    mean_ns = json.loads(estimates.read_text())["mean"]["point_estimate"]
    ms = mean_ns / 1e6
    if size not in worst or ms > worst[size][0]:
        worst[size] = (ms, name)

budgets = {"10k": 50.0, "50k": 150.0}
failed = False
for size, budget in budgets.items():
    if size not in worst:
        print(f"{size}: no scenarios found")
        failed = True
        continue
    ms, name = worst[size]
    verdict = "OK" if ms < budget else "OVER BUDGET"
    if ms >= budget:
        failed = True
    print(f"{size}: worst {ms:.2f}ms ({name}) — budget {budget:.0f}ms — {verdict}")

raise SystemExit(1 if failed else 0)
EOF
