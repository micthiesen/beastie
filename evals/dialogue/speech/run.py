#!/usr/bin/env python3
"""Run learned-word speech cases through the real worker and grade them.

usage: run.py SPLIT LABEL [--backend llama-server] [--model PATH]
Writes results/LABEL-SPLIT.json and prints the summary. The worker binary must be built.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
SOUND = re.compile(r"^(m+r*p*|p+r+p*|b+w+e+p*|e+p*|h+m+p*|o+|n+|z+|r+|b+r+|u+h*|m+p+)$")


def words(line):
    return [w for w in re.split(r"[^a-z']+", line.lower()) if w]


def run(cases, backend, model):
    payload = "".join(json.dumps(case["request"]) + "\n" for case in cases)
    command = [str(ROOT / "target/debug/beastie-ai-worker"), "--backend", backend]
    if model:
        command += ["--model", model]
    env = dict(__import__("os").environ, BEASTIE_DEBUG_MODEL="1")
    out = subprocess.run(command, input=payload, capture_output=True, text=True, timeout=1800, env=env)
    raw = {}
    for line in out.stderr.splitlines():
        match = re.match(r"DEBUG speech raw (\d+): (.*)", line)
        if match:
            raw.setdefault(int(match.group(1)), []).append(json.loads(match.group(2)))
    replies = [json.loads(line) for line in out.stdout.splitlines() if line.strip()]
    for reply in replies:
        reply["raw"] = raw.get(reply["request_id"], [])
    return replies


def grade(case, reply, composed):
    request = case["request"]
    intent = request["speech_intent"]
    vocab = {entry["word"]: entry["meaning"] for entry in request["vocabulary"]}
    said = words(reply["say"])
    valid = reply.get("worker_fallback") is None
    target = None
    kind = intent["kind"]
    if kind == "new_word":
        target = intent["word"]
    elif kind == "answer" and intent["response"] in ("comply", "refuse", "delight", "look"):
        target = intent["word"]
    elif kind in ("want", "remark"):
        matches = [w for w, m in vocab.items() if m == intent["meaning"]]
        target = matches[-1] if matches else None
    elif kind == "echo":
        target = intent["attempt"].rstrip("?")
    uses_word = target is None or target in said
    if kind == "babble":
        uses_word = all(SOUND.match(w) for w in said)
    refusal_marked = True
    if kind == "answer" and intent["response"] == "refuse":
        refusal_marked = any(w == "no" or w.startswith("nn") for w in said)
    good = valid and uses_word and refusal_marked
    return {
        "id": case["id"],
        "say": reply["say"],
        "composed": composed["say"],
        "valid": valid,
        "uses_word": uses_word,
        "refusal_marked": refusal_marked,
        "good": good,
        "copies_composer": reply["say"].strip().lower() == composed["say"].strip().lower(),
        "raw": reply.get("raw", []),
    }


def main():
    split, label = sys.argv[1], sys.argv[2]
    backend = "llama-server"
    model = str(ROOT / "models/Qwen3.5-0.8B-Q4_0.gguf")
    args = sys.argv[3:]
    if "--backend" in args:
        backend = args[args.index("--backend") + 1]
    if "--model" in args:
        model = args[args.index("--model") + 1]
    cases = [json.loads(line) for line in (HERE / f"cases_{split}.jsonl").read_text().splitlines()]
    replies = run(cases, backend, model if backend != "fixture" else None)
    composed = run(cases, "fixture", None)
    graded = [grade(c, r, k) for c, r, k in zip(cases, replies, composed)]
    model_lines = [g["say"] for g in graded if g["valid"]]
    summary = {
        "label": label,
        "split": split,
        "backend": backend,
        "cases": len(graded),
        "good": sum(g["good"] for g in graded),
        "valid": sum(g["valid"] for g in graded),
        "uses_word": sum(g["uses_word"] for g in graded),
        "distinct_model_lines": len(set(model_lines)),
        "copies_composer": sum(g["copies_composer"] for g in graded if g["valid"]),
    }
    out = HERE / "results" / f"{label}-{split}.json"
    out.parent.mkdir(exist_ok=True)
    out.write_text(json.dumps({"summary": summary, "cases": graded}, indent=1))
    print(json.dumps(summary))
    for g in graded:
        flag = "ok " if g["good"] else "BAD"
        raw = "" if g["good"] else f" raw={g['raw']}"
        print(f"{flag} {g['id']:<28} {g['say']!r:<28} composer={g['composed']!r}{raw}")


if __name__ == "__main__":
    main()
