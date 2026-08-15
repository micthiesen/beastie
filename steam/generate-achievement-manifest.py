#!/usr/bin/env python3
"""Validate and canonicalize deterministic Steam achievement IDs."""
import argparse
import json
import pathlib
import re

PATTERN = re.compile(r"^BEASTIE_[A-Z0-9_]+$")

def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()
    document = json.loads(args.source.read_text(encoding="utf-8"))
    if document.get("version") != 1:
        raise SystemExit("achievement manifest version must be 1")
    achievements = document.get("achievements")
    if not isinstance(achievements, list) or not achievements:
        raise SystemExit("achievement manifest must contain a non-empty list")
    ids = [entry.get("id") for entry in achievements]
    if any(not isinstance(identifier, str) or not PATTERN.fullmatch(identifier) for identifier in ids):
        raise SystemExit("achievement IDs must be deterministic BEASTIE_* identifiers")
    if len(ids) != len(set(ids)):
        raise SystemExit("achievement IDs must be unique")
    canonical = {"version": 1, "achievements": sorted(achievements, key=lambda entry: entry["id"])}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(canonical, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

if __name__ == "__main__":
    main()
