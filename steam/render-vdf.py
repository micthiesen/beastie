#!/usr/bin/env python3
"""Render the small Steam VDF templates from environment variables."""
import os
import pathlib
import re
import sys

TOKEN = re.compile(r"\$\{([A-Z0-9_]+)\}")

if len(sys.argv) != 3:
    raise SystemExit("usage: render-vdf.py <template> <output>")
source = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")

def replace(match: re.Match[str]) -> str:
    name = match.group(1)
    value = os.environ.get(name)
    if value is None:
        raise SystemExit(f"missing environment variable {name}")
    return value

rendered = TOKEN.sub(replace, source)
pathlib.Path(sys.argv[2]).write_text(rendered, encoding="utf-8")
