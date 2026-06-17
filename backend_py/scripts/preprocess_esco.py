"""
Preprocess ESCO Greek skills CSV → compact JSON used by the backend.

Reads:  ESCO dataset - v1.2.1 - classification - el - csv/skills_el.csv
Writes: backend_py/src/edu_backend/data/skills_compact.json

Output schema (v2 — adds the ESCO URI):

    {
      "<preferredLabel lowercased>": {
        "uri": "http://data.europa.eu/esco/skill/<uuid>",
        "description": "<Greek description>"
      },
      ...
    }

Run from the project root:

    python backend_py/scripts/preprocess_esco.py
"""
import csv
import json
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[2]
CSV_PATH = PROJECT_ROOT / "ESCO dataset - v1.2.1 - classification - el - csv" / "skills_el.csv"
OUT_PATH = PROJECT_ROOT / "backend_py" / "src" / "edu_backend" / "data" / "skills_compact.json"


def main() -> int:
    if not CSV_PATH.exists():
        print(f"ERROR: CSV not found at {CSV_PATH}", file=sys.stderr)
        return 1

    out: dict[str, dict[str, str]] = {}
    dupes = 0

    with CSV_PATH.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            label = (row.get("preferredLabel") or "").strip()
            uri = (row.get("conceptUri") or "").strip()
            description = (row.get("description") or "").strip()
            if not label or not uri:
                continue
            key = label.lower()
            if key in out:
                dupes += 1
                continue
            out[key] = {"uri": uri, "description": description}

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with OUT_PATH.open("w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=None, separators=(",", ":"))

    size_kb = OUT_PATH.stat().st_size / 1024
    print(f"Wrote {len(out)} skills → {OUT_PATH} ({size_kb:.1f} KB)")
    if dupes:
        print(f"Skipped {dupes} duplicate labels (first occurrence kept)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
