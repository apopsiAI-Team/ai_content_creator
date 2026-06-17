"""
ESCO Skills Service - Loads and lookups skills from compact JSON.

Compact JSON format (v2):
    { "<preferredLabel lowercased>": { "uri": "...", "description": "..." } }

The loader also accepts the legacy v1 format ({ name → "description" }) for
backward compatibility; in that case `uri` is returned as an empty string.
"""
import json
from pathlib import Path
from typing import Optional


class ESCOService:
    def __init__(self, data_path: Path):
        # name_lowercased → { "uri": str, "description": str }
        self.skills: dict[str, dict[str, str]] = {}
        self._load_skills(data_path)

    def _load_skills(self, data_path: Path) -> None:
        """Load skills from JSON file (supports v2 and legacy v1 formats)."""
        if not data_path.exists():
            print(f"Warning: ESCO skills file not found at {data_path}")
            return

        with open(data_path, "r", encoding="utf-8") as f:
            raw = json.load(f)

        normalized: dict[str, dict[str, str]] = {}
        for name, value in raw.items():
            if isinstance(value, dict):
                # v2 format
                normalized[name] = {
                    "uri": value.get("uri", "") or "",
                    "description": value.get("description", "") or "",
                }
            else:
                # legacy v1 — value was a description string
                normalized[name] = {
                    "uri": "",
                    "description": value or "",
                }
        self.skills = normalized
        print(f"Loaded {len(self.skills)} ESCO skills")

    def lookup(self, skill_names: list[str]) -> list[dict]:
        """Lookup uri + description for skill names.

        Returns one entry per input name. If a name isn't found, uri and
        description are empty strings (never None) so downstream code can
        treat the shape uniformly.
        """
        results = []
        for name in skill_names:
            normalized = name.lower().strip()
            entry = self.skills.get(normalized) or {}
            results.append({
                "name": name,
                "uri": entry.get("uri", "") or "",
                "description": entry.get("description", "") or "",
            })
        return results

    def lookup_one(self, skill_name: str) -> dict:
        """Convenience: lookup a single skill, returns { name, uri, description }."""
        return self.lookup([skill_name])[0]

    def search(self, query: str, limit: int = 10) -> list[dict]:
        """Search skills by partial match on name."""
        query_lower = query.lower()
        matches = []
        for name, entry in self.skills.items():
            if query_lower in name:
                matches.append({
                    "name": name,
                    "uri": entry.get("uri", "") or "",
                    "description": entry.get("description", "") or "",
                })
                if len(matches) >= limit:
                    break
        return matches


# Singleton instance
_esco_service: Optional[ESCOService] = None


def get_esco_service(data_path: Path) -> ESCOService:
    global _esco_service
    if _esco_service is None:
        _esco_service = ESCOService(data_path)
    return _esco_service
